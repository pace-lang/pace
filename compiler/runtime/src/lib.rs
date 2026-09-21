#![no_std]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

extern crate alloc;

use alloc::alloc::{Layout, alloc, dealloc, handle_alloc_error};
use core::ffi::{c_char, c_double, c_int, c_longlong, c_void};
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

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        printf(c"Rust panic!\n".as_ptr());
        abort();
    }
}

#[cfg(not(test))]
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
        printf(c"Fatal error: %s\n".as_ptr(), msg);
        abort();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_alloc(
    size: usize,
    deinit: Option<unsafe extern "C" fn(*mut c_void)>,
) -> *mut c_void {
    let header_size = core::mem::size_of::<ArcHeader>();
    let total_size = header_size + size;

    let layout = Layout::from_size_align(total_size, core::mem::align_of::<ArcHeader>())
        .unwrap_or_else(|_| {
            pace_panic(c"Invalid allocation layout".as_ptr());
        });

    unsafe {
        let ptr = alloc(layout);
        if ptr.is_null() {
            handle_alloc_error(layout);
        }

        let header = ptr as *mut ArcHeader;
        core::ptr::write(
            header,
            ArcHeader {
                ref_count: AtomicUsize::new(1),
                size,
                deinit,
            },
        );

        ptr.add(header_size) as *mut c_void
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
        let layout =
            Layout::from_size_align(total_size, core::mem::align_of::<ArcHeader>()).unwrap();
        dealloc(header_ptr as *mut u8, layout);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_print_int(val: c_longlong) {
    unsafe {
        printf(c"%lld\n".as_ptr(), val);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_print_float(val: c_double) {
    unsafe {
        printf(c"%f\n".as_ptr(), val);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_print_bool(val: c_int) {
    if val != 0 {
        unsafe {
            printf(c"true\n".as_ptr());
        }
    } else {
        unsafe {
            printf(c"false\n".as_ptr());
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_println() {
    unsafe {
        printf(c"\n".as_ptr());
    }
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
        let obj_ptr =
            pace_alloc(core::mem::size_of::<PaceString>(), Some(string_deinit)) as *mut PaceString;

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
        unsafe {
            printf(c"null\n".as_ptr());
        }
        return;
    }
    unsafe {
        printf(c"%s\n".as_ptr(), (*val).data);
    }
}
