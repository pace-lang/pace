#![allow(clippy::missing_safety_doc)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU64, Ordering};

unsafe extern "C" {
    fn calloc(nobj: usize, size: usize) -> *mut u8;
    fn free(p: *mut u8);
}

#[repr(C)]
pub struct PaceClassMetadata {
    pub deinit_fn: *const (),
    pub field_count: u64,
    pub field_offsets: [u64; 0],
}

#[unsafe(no_mangle)]
pub extern "C" fn printInt(value: i64) -> i64 {
    println!("{}", value);
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn printFloat(value: f64) -> i64 {
    println!("{}", value);
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn printBool(value: i64) -> i64 {
    if value == 0 {
        println!("false");
    } else {
        println!("true");
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn printStr(str_ptr: *const c_char) -> i64 {
    if str_ptr.is_null() {
        println!("(null)");
        return 0;
    }
    let c_str = unsafe { CStr::from_ptr(str_ptr.add(24)) };
    match c_str.to_str() {
        Ok(s) => {
            println!("{}", s);
        }
        Err(_) => {
            println!("{:?}", c_str);
        }
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_panic(code: i64) {
    if code == 1 {
        println!("Pace Runtime Error: Attempted to unwrap a null Optional");
    } else if code == 2 {
        println!("Pace Runtime Error: Array index out of bounds");
    } else {
        println!("Pace Runtime Error: Code {}", code);
    }
    std::process::exit(1);
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_alloc(size: i64, metadata_ptr: *const ()) -> *mut u8 {
    let ptr = unsafe { calloc(1, size as usize) };
    if ptr.is_null() {
        println!("Pace Runtime Error: Out of memory");
        std::process::exit(1);
    }

    // Set strong reference count to 1 (offset 0)
    unsafe {
        *(ptr as *mut u64) = 1;
    }
    // Set weak reference count to 1 (offset 8)
    unsafe {
        *(ptr.add(8) as *mut u64) = 1;
    }
    // Set type metadata (offset 16)
    unsafe {
        *(ptr.add(16) as *mut u64) = metadata_ptr as u64;
    }

    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_alloc_array_repeat(count: u64, val: u64, metadata_val: u64) -> *mut u8 {
    let total_size = 32 + count * 8;
    let ptr = pace_alloc(total_size as i64, metadata_val as *const ());
    unsafe {
        *(ptr.add(24) as *mut u64) = count;
        for i in 0..count {
            *(ptr.add(32 + (i as usize) * 8) as *mut u64) = val;
        }
    }
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_retain(obj: *mut u8) {
    if obj.is_null() {
        return;
    }
    let strong_count = unsafe { &*(obj as *const AtomicU64) };
    if strong_count.load(Ordering::Relaxed) == 0x7FFFFFFF_FFFFFFFF {
        return;
    }
    strong_count.fetch_add(1, Ordering::SeqCst);
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_weak_release(obj: *mut u8) {
    if obj.is_null() {
        return;
    }
    let strong_count = unsafe { &*(obj as *const AtomicU64) };
    if strong_count.load(Ordering::Relaxed) == 0x7FFFFFFF_FFFFFFFF {
        return;
    }
    let weak_count = unsafe { &*(obj.add(8) as *const AtomicU64) };
    let old_count = weak_count.fetch_sub(1, Ordering::SeqCst);
    if old_count == 1 {
        unsafe { free(obj) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_release(obj: *mut u8) {
    if obj.is_null() {
        return;
    }
    let strong_count = unsafe { &*(obj as *const AtomicU64) };
    if strong_count.load(Ordering::Relaxed) == 0x7FFFFFFF_FFFFFFFF {
        return;
    }
    let old_count = strong_count.fetch_sub(1, Ordering::SeqCst);
    if old_count == 1 {
        let metadata_val = unsafe { *(obj.add(16) as *const u64) };
        if metadata_val == !0 {
            // -1 as u64
            let length = unsafe { *(obj.add(24) as *const u64) };
            for i in 0..length {
                let element = unsafe { *(obj.add(32 + (i as usize) * 8) as *const *mut u8) };
                if !element.is_null() {
                    pace_release(element);
                }
            }
        } else if metadata_val != !1 && metadata_val != 0 {
            // !1 is -2 as u64
            let metadata = metadata_val as *const PaceClassMetadata;

            let deinit_fn = unsafe { (*metadata).deinit_fn };
            if !deinit_fn.is_null() {
                let deinit: extern "C" fn(*mut u8) = unsafe { std::mem::transmute(deinit_fn) };
                deinit(obj);
            }

            let field_count = unsafe { (*metadata).field_count };
            for i in 0..field_count {
                let offset = unsafe { *(*metadata).field_offsets.as_ptr().add(i as usize) };
                let field_ptr = unsafe { *(obj.add(offset as usize) as *const *mut u8) };
                if !field_ptr.is_null()
                    && (field_ptr as usize).is_multiple_of(8)
                    && (field_ptr as usize) > 0x10000
                {
                    pace_release(field_ptr);
                }
            }
        }

        pace_weak_release(obj);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_weak_retain(obj: *mut u8) {
    if obj.is_null() {
        return;
    }
    let strong_count = unsafe { &*(obj as *const AtomicU64) };
    if strong_count.load(Ordering::Relaxed) == 0x7FFFFFFF_FFFFFFFF {
        return;
    }
    let weak_count = unsafe { &*(obj.add(8) as *const AtomicU64) };
    weak_count.fetch_add(1, Ordering::SeqCst);
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_weak_upgrade(obj: *mut u8) -> *mut u8 {
    if obj.is_null() {
        return std::ptr::null_mut();
    }
    let strong_count = unsafe { &*(obj as *const AtomicU64) };
    if strong_count.load(Ordering::Relaxed) == 0x7FFFFFFF_FFFFFFFF {
        return obj;
    }

    let mut count = strong_count.load(Ordering::SeqCst);
    while count > 0 {
        match strong_count.compare_exchange_weak(
            count,
            count + 1,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => return obj,
            Err(x) => count = x,
        }
    }
    std::ptr::null_mut()
}

// ---------------------------------------------------------
// FFI String Operations
// ---------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn stringLen(s_ptr: *const c_char) -> i64 {
    if s_ptr.is_null() {
        return 0;
    }
    let c_str = unsafe { CStr::from_ptr(s_ptr.add(24)) };
    c_str.to_bytes().len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn stringConcat(a_ptr: *const c_char, b_ptr: *const c_char) -> *const c_char {
    let a_bytes = if a_ptr.is_null() {
        &[]
    } else {
        unsafe { CStr::from_ptr(a_ptr.add(24)) }.to_bytes()
    };
    let b_bytes = if b_ptr.is_null() {
        &[]
    } else {
        unsafe { CStr::from_ptr(b_ptr.add(24)) }.to_bytes()
    };

    let total_len = a_bytes.len() + b_bytes.len();

    // Allocate space for the combined string + null terminator + 24 byte header
    let new_ptr = pace_alloc((24 + total_len + 1) as i64, (-2isize) as *const ());
    if new_ptr.is_null() {
        println!("Pace Runtime Error: Out of memory in stringConcat");
        std::process::exit(1);
    }

    // Copy bytes to payload area (offset 24)
    unsafe {
        let payload_ptr = new_ptr.add(24);
        std::ptr::copy_nonoverlapping(a_bytes.as_ptr(), payload_ptr, a_bytes.len());
        std::ptr::copy_nonoverlapping(
            b_bytes.as_ptr(),
            payload_ptr.add(a_bytes.len()),
            b_bytes.len(),
        );
        // null terminator already set by calloc inside pace_alloc
    }

    new_ptr as *const c_char
}

#[unsafe(no_mangle)]
pub extern "C" fn stringSubstring(s_ptr: *const c_char, start: i64, end: i64) -> *const c_char {
    if s_ptr.is_null() {
        return std::ptr::null();
    }
    let s_bytes = unsafe { CStr::from_ptr(s_ptr.add(24)) }.to_bytes();

    let len = s_bytes.len() as i64;
    let mut safe_start = start;
    let mut safe_end = end;

    if safe_start < 0 {
        safe_start = 0;
    }
    if safe_end > len {
        safe_end = len;
    }
    if safe_start > safe_end {
        safe_start = safe_end;
    }

    let slice_len = (safe_end - safe_start) as usize;
    let new_ptr = pace_alloc((24 + slice_len + 1) as i64, (-2isize) as *const ());
    if new_ptr.is_null() {
        println!("Pace Runtime Error: Out of memory in stringSubstring");
        std::process::exit(1);
    }

    unsafe {
        let payload_ptr = new_ptr.add(24);
        std::ptr::copy_nonoverlapping(
            s_bytes.as_ptr().add(safe_start as usize),
            payload_ptr,
            slice_len,
        );
    }

    new_ptr as *const c_char
}

#[unsafe(no_mangle)]
pub extern "C" fn stringContains(s_ptr: *const c_char, sub_ptr: *const c_char) -> i64 {
    if s_ptr.is_null() || sub_ptr.is_null() {
        return 0; // false
    }

    let s_str = unsafe { CStr::from_ptr(s_ptr.add(24)) }.to_string_lossy();
    let sub_str = unsafe { CStr::from_ptr(sub_ptr.add(24)) }.to_string_lossy();

    if s_str.contains(sub_str.as_ref()) {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fileIsValid(_ptr: *mut u8) -> u8 {
    0 // false
}

#[unsafe(no_mangle)]
pub extern "C" fn fileReadAll(_ptr: *mut u8) -> *mut u8 {
    std::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn fileClose(_ptr: *mut u8) {}

#[unsafe(no_mangle)]
pub extern "C" fn fileWrite(_ptr: *mut u8, _data: *mut u8) {}

#[unsafe(no_mangle)]
pub extern "C" fn fileOpen(_path: *mut u8) -> *mut u8 {
    std::ptr::null_mut()
}
#[unsafe(no_mangle)]
pub extern "C" fn pace_string_concat(s1: *const c_char, s2: *const c_char) -> *const c_char {
    if s1.is_null() && s2.is_null() {
        return std::ptr::null();
    }

    let str1 = if s1.is_null() {
        ""
    } else {
        unsafe { CStr::from_ptr(s1.add(24)) }.to_str().unwrap_or("")
    };
    let str2 = if s2.is_null() {
        ""
    } else {
        unsafe { CStr::from_ptr(s2.add(24)) }.to_str().unwrap_or("")
    };

    let total_len = str1.len() + str2.len();
    let ptr = pace_alloc((24 + total_len + 1) as i64, !1_u64 as *const ());

    unsafe {
        let payload = ptr.add(24);
        std::ptr::copy_nonoverlapping(str1.as_ptr(), payload, str1.len());
        std::ptr::copy_nonoverlapping(str2.as_ptr(), payload.add(str1.len()), str2.len());
        *payload.add(total_len) = 0; // null terminator
    }

    ptr as *const c_char
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_int_to_string(value: i64) -> *const c_char {
    let s = format!("{}", value);
    let ptr = pace_alloc((24 + s.len() + 1) as i64, !1_u64 as *const ());
    unsafe {
        let payload = ptr.add(24);
        std::ptr::copy_nonoverlapping(s.as_ptr(), payload, s.len());
        *payload.add(s.len()) = 0;
    }
    ptr as *const c_char
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_float_to_string(value: f64) -> *const c_char {
    let s = format!("{}", value);
    let ptr = pace_alloc((24 + s.len() + 1) as i64, !1_u64 as *const ());
    unsafe {
        let payload = ptr.add(24);
        std::ptr::copy_nonoverlapping(s.as_ptr(), payload, s.len());
        *payload.add(s.len()) = 0;
    }
    ptr as *const c_char
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_bool_to_string(value: i64) -> *const c_char {
    let s = if value != 0 { "true" } else { "false" };
    let ptr = pace_alloc((24 + s.len() + 1) as i64, !1_u64 as *const ());
    unsafe {
        let payload = ptr.add(24);
        std::ptr::copy_nonoverlapping(s.as_ptr(), payload, s.len());
        *payload.add(s.len()) = 0;
    }
    ptr as *const c_char
}

#[unsafe(no_mangle)]
pub extern "C" fn printEnum(val: *const u8) {
    if val.is_null() {
        println!("null");
        return;
    }
    unsafe {
        let metadata_ptr = *(val.add(16) as *const *const std::os::raw::c_char);
        let tag = *(val.add(24) as *const i64);
        let enum_name = if metadata_ptr.is_null() {
            "Enum"
        } else {
            std::ffi::CStr::from_ptr(metadata_ptr)
                .to_str()
                .unwrap_or("Enum")
        };
        println!("<{} Variant {}>", enum_name, tag);
    }
}
