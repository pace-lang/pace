#![no_std]

extern crate alloc;

use alloc::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use core::ffi::{c_char, c_void, c_longlong, c_double, c_int};
use core::sync::atomic::{AtomicUsize, Ordering};
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

unsafe extern "C" {
    fn printf(format: *const c_char, ...) -> c_int;
    fn strlen(s: *const c_char) -> usize;
    fn abort() -> !;
    fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        printf(b"Rust panic!\n\0".as_ptr() as *const c_char);
        abort();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_eh_personality() {}

#[repr(C)]
pub struct ArcHeader {
    pub ref_count: AtomicUsize,
    pub size: usize,
    pub deinit: Option<unsafe extern "C" fn(*mut c_void)>,
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_panic(msg: *const c_char) -> ! {
    unsafe {
        printf(b"Fatal error: %s\n\0".as_ptr() as *const c_char, msg);
        abort();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_alloc(size: usize, deinit: Option<unsafe extern "C" fn(*mut c_void)>) -> *mut c_void {
    let header_size = core::mem::size_of::<ArcHeader>();
    let total_size = header_size + size;
    
    let layout = Layout::from_size_align(total_size, core::mem::align_of::<ArcHeader>()).unwrap_or_else(|_| {
        pace_panic(b"Invalid allocation layout\0".as_ptr() as *const c_char);
    });

    unsafe {
        let ptr = alloc(layout);
        if ptr.is_null() {
            handle_alloc_error(layout);
        }

        let header = ptr as *mut ArcHeader;
        core::ptr::write(header, ArcHeader {
            ref_count: AtomicUsize::new(1),
            size,
            deinit,
        });

        (ptr as *mut u8).add(header_size) as *mut c_void
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_retain(obj: *mut c_void) {
    if obj.is_null() {
        return;
    }

    unsafe {
        let header_ptr = (obj as *mut u8).sub(core::mem::size_of::<ArcHeader>()) as *mut ArcHeader;
        (*header_ptr).ref_count.fetch_add(1, Ordering::Relaxed);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_release(obj: *mut c_void) {
    if obj.is_null() {
        return;
    }

    unsafe {
        let header_ptr = (obj as *mut u8).sub(core::mem::size_of::<ArcHeader>()) as *mut ArcHeader;
        
        if (*header_ptr).ref_count.fetch_sub(1, Ordering::Release) != 1 {
            return;
        }

        core::sync::atomic::fence(Ordering::Acquire);

        if let Some(deinit) = (*header_ptr).deinit {
            deinit(obj);
        }

        let total_size = core::mem::size_of::<ArcHeader>() + (*header_ptr).size;
        let layout = Layout::from_size_align(total_size, core::mem::align_of::<ArcHeader>()).unwrap();
        dealloc(header_ptr as *mut u8, layout);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_print_int(val: c_longlong) {
    unsafe { printf(b"%lld\n\0".as_ptr() as *const c_char, val); }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_print_float(val: c_double) {
    unsafe { printf(b"%f\n\0".as_ptr() as *const c_char, val); }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_print_bool(val: c_int) {
    if val != 0 {
        unsafe { printf(b"true\n\0".as_ptr() as *const c_char); }
    } else {
        unsafe { printf(b"false\n\0".as_ptr() as *const c_char); }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_println() {
    unsafe { printf(b"\n\0".as_ptr() as *const c_char); }
}

#[repr(C)]
pub struct PaceString {
    pub data: *mut c_char,
    pub length: usize,
}

extern "C" fn string_deinit(obj: *mut c_void) {
    unsafe {
        let string_obj = obj as *mut PaceString;
        let layout = Layout::from_size_align((*string_obj).length + 1, 1).unwrap();
        dealloc((*string_obj).data as *mut u8, layout);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_string_new(c_str: *const c_char) -> *mut PaceString {
    unsafe {
        let len = strlen(c_str);
        let obj_ptr = pace_alloc(core::mem::size_of::<PaceString>(), Some(string_deinit)) as *mut PaceString;
        
        let layout = Layout::from_size_align(len + 1, 1).unwrap();
        let data_ptr = alloc(layout) as *mut c_char;
        
        memcpy(data_ptr as *mut c_void, c_str as *const c_void, len + 1);
        
        (*obj_ptr).data = data_ptr;
        (*obj_ptr).length = len;
        
        obj_ptr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_print_string(val: *mut PaceString) {
    if val.is_null() {
        unsafe { printf(b"null\n\0".as_ptr() as *const c_char); }
        return;
    }
    unsafe {
        printf(b"%s\n\0".as_ptr() as *const c_char, (*val).data);
    }
}
