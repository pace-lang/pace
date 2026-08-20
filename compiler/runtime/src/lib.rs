#![allow(clippy::missing_safety_doc)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
pub mod async_rt;
pub mod actor;
use std::ffi::{c_void, c_char, CStr};
use std::sync::atomic::{AtomicU64, Ordering};

unsafe extern "C" {
    fn calloc(nobj: usize, size: usize) -> *mut u8;
    fn free(p: *mut u8);
}

#[repr(C)]
pub struct PaceClassMetadata {
    pub deinit_fn: *const (),
    pub mailbox_offset: u64,
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
pub extern "C" fn paceStringFromBytes(bytes: *const u8, len: i64) -> *const c_char {
    let new_ptr = pace_alloc(24 + len + 1, (-2isize) as *const ());
    unsafe {
        let payload_ptr = new_ptr.add(24);
        std::ptr::copy_nonoverlapping(bytes, payload_ptr, len as usize);
        *payload_ptr.add(len as usize) = 0; // null terminator
    }
    new_ptr as *const c_char
}

#[unsafe(no_mangle)]
pub extern "C" fn debug_ptr(ptr: *mut u64) {
    if ptr.is_null() {
        println!("debug_ptr: NULL");
        return;
    }
    unsafe {
        println!("debug_ptr: {:p}", ptr);
        for i in 0..10 {
            println!("  offset {}: {}", i * 8, *ptr.add(i));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pacePanic(code: i64) {
    if code == 1 {
        println!("Pace Runtime Error: Attempted to unwrap a null Optional");
    } else if code == 2 {
        println!("Pace Runtime Error: Array index out of bounds");
    } else if code == 3 {
        println!("Pace Runtime Error: Attempted to unwrap an Err Result");
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

    // Initialize header
    unsafe {
        *(ptr as *mut i64) = 1; // strong count
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

            let mailbox_offset = unsafe { (*metadata).mailbox_offset };
            if mailbox_offset > 0 {
                // Destroy the mailbox
                let mailbox_ptr = unsafe { *(obj.add(mailbox_offset as usize) as *const *mut c_void) };
                if !mailbox_ptr.is_null() {
                    unsafe extern "C" {
                        fn pace_actor_mailbox_destroy(mailbox: *mut c_void);
                    }
                    unsafe {
                        pace_actor_mailbox_destroy(mailbox_ptr);
                    }
                }
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
    if s_ptr.is_null() || start < 0 || end < start {
        return allocate_pace_string("");
    }
    
    let safe_start = start;
    let safe_end = end;
    let slice_len = (safe_end - safe_start) as usize;
    
    let new_ptr = pace_alloc((24 + slice_len + 1) as i64, (-2isize) as *const ());
    if new_ptr.is_null() {
        println!("Pace Runtime Error: Out of memory in stringSubstring");
        std::process::exit(1);
    }

    unsafe {
        let payload_ptr = new_ptr.add(24);
        let src_ptr = s_ptr.add(24);
        
        let mut actual_len = 0;
        for i in 0..slice_len {
            let b = *(src_ptr.add(safe_start as usize + i) as *const u8);
            if b == 0 {
                break;
            }
            *(payload_ptr.add(i)) = b;
            actual_len += 1;
        }
        
        *(payload_ptr.add(actual_len)) = 0;
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

fn allocate_pace_string(s: &str) -> *const c_char {
    let bytes = s.as_bytes();
    let new_ptr = pace_alloc((24 + bytes.len() + 1) as i64, (-2isize) as *const ());
    if new_ptr.is_null() {
        println!("Pace Runtime Error: Out of memory in string allocation");
        std::process::exit(1);
    }
    unsafe {
        let payload_ptr = new_ptr.add(24);
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), payload_ptr, bytes.len());
        *payload_ptr.add(bytes.len()) = 0; // Null terminator
    }
    new_ptr as *const c_char
}

#[unsafe(no_mangle)]
pub extern "C" fn stringSplit(
    s_ptr: *const c_char,
    sub_ptr: *const c_char,
) -> *mut core::ffi::c_void {
    if s_ptr.is_null() {
        let vec: Vec<*mut core::ffi::c_void> = Vec::new();
        return Box::into_raw(Box::new(vec)) as *mut core::ffi::c_void;
    }
    let s_str = unsafe { CStr::from_ptr(s_ptr.add(24)) }.to_string_lossy();
    let mut vec: Vec<*mut core::ffi::c_void> = Vec::new();

    if sub_ptr.is_null() {
        vec.push(allocate_pace_string(s_str.as_ref()) as *mut core::ffi::c_void);
    } else {
        let sub_str = unsafe { CStr::from_ptr(sub_ptr.add(24)) }.to_string_lossy();
        for part in s_str.split(sub_str.as_ref()) {
            vec.push(allocate_pace_string(part) as *mut core::ffi::c_void);
        }
    }
    Box::into_raw(Box::new(vec)) as *mut core::ffi::c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn stringSplitLen(ptr: *mut core::ffi::c_void) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    let vec = unsafe { &mut *(ptr as *mut Vec<*mut core::ffi::c_void>) };
    vec.len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn stringSplitAt(ptr: *mut core::ffi::c_void, index: i64) -> *mut core::ffi::c_void {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let vec = unsafe { &mut *(ptr as *mut Vec<*mut core::ffi::c_void>) };
    if index < 0 || index as usize >= vec.len() {
        return std::ptr::null_mut();
    }
    vec[index as usize]
}

#[unsafe(no_mangle)]
pub extern "C" fn stringSplitFree(ptr: *mut core::ffi::c_void) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr as *mut Vec<*mut core::ffi::c_void>)) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn stringReplace(
    s_ptr: *const c_char,
    old_ptr: *const c_char,
    new_ptr: *const c_char,
) -> *const c_char {
    if s_ptr.is_null() {
        return std::ptr::null();
    }
    if old_ptr.is_null() || new_ptr.is_null() {
        return s_ptr;
    }
    let s_str = unsafe { CStr::from_ptr(s_ptr.add(24)) }.to_string_lossy();
    let old_str = unsafe { CStr::from_ptr(old_ptr.add(24)) }.to_string_lossy();
    let new_str = unsafe { CStr::from_ptr(new_ptr.add(24)) }.to_string_lossy();

    let replaced = s_str.replace(old_str.as_ref(), new_str.as_ref());
    allocate_pace_string(&replaced)
}

#[unsafe(no_mangle)]
pub extern "C" fn stringTrim(s_ptr: *const c_char) -> *const c_char {
    if s_ptr.is_null() {
        return std::ptr::null();
    }
    let s_str = unsafe { CStr::from_ptr(s_ptr.add(24)) }.to_string_lossy();
    let trimmed = s_str.trim();
    allocate_pace_string(trimmed)
}

#[unsafe(no_mangle)]
pub extern "C" fn stringToLower(s_ptr: *const c_char) -> *const c_char {
    if s_ptr.is_null() {
        return allocate_pace_string("");
    }
    let s_str = unsafe { CStr::from_ptr(s_ptr.add(24)) }.to_string_lossy();
    let lower = s_str.to_lowercase();
    allocate_pace_string(&lower)
}

#[unsafe(no_mangle)]
pub extern "C" fn stringCharAt(s: *const c_char, index: i64) -> i64 {
    if s.is_null() || index < 0 {
        return 0;
    }
    unsafe {
        let str_ptr = s.add(24);
        let b = *(str_ptr.add(index as usize) as *const u8);
        if b == 0 {
            return 0;
        }
        b as i64
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn stringSkipWhitespace(s: *const c_char, start: i64) -> i64 {
    if s.is_null() || start < 0 { return start; }
    unsafe {
        let mut i = start as usize;
        let str_ptr = s.add(24);
        loop {
            let b = *(str_ptr.add(i) as *const u8);
            if b == b' ' || b == b'\n' || b == b'\r' || b == b'\t' {
                i += 1;
            } else {
                return i as i64;
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn stringFindStringEnd(s: *const c_char, start: i64) -> i64 {
    if s.is_null() || start < 0 { return -1; }
    unsafe {
        let mut i = start as usize;
        let str_ptr = s.add(24);
        loop {
            let b = *(str_ptr.add(i) as *const u8);
            if b == 0 { break; }
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                return i as i64;
            }
            i += 1;
        }
        -1
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn stringToInt(s_ptr: *const c_char) -> i64 {
    if s_ptr.is_null() {
        return 0;
    }
    let s_str = unsafe { CStr::from_ptr(s_ptr.add(24)) }.to_string_lossy();
    s_str.trim().parse::<i64>().unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn stringToFloat(s_ptr: *const c_char) -> f64 {
    if s_ptr.is_null() {
        return 0.0;
    }
    let s_str = unsafe { CStr::from_ptr(s_ptr.add(24)) }.to_string_lossy();
    s_str.trim().parse::<f64>().unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn hash_String(s_ptr: *const c_char) -> i64 {
    if s_ptr.is_null() {
        return 0;
    }
    let s_str = unsafe { CStr::from_ptr(s_ptr.add(24)) }.to_string_lossy();
    
    // FNV-1a hash algorithm
    let mut hash: u64 = 14695981039346656037;
    for byte in s_str.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn equals_String(a_ptr: *const c_char, b_ptr: *const c_char) -> i64 {
    if a_ptr == b_ptr {
        return 1;
    }
    if a_ptr.is_null() || b_ptr.is_null() {
        return 0;
    }
    let a_str = unsafe { CStr::from_ptr(a_ptr.add(24)) }.to_string_lossy();
    let b_str = unsafe { CStr::from_ptr(b_ptr.add(24)) }.to_string_lossy();
    if a_str == b_str { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn stringToUpper(s_ptr: *const c_char) -> *const c_char {
    if s_ptr.is_null() {
        return std::ptr::null();
    }
    let s_str = unsafe { CStr::from_ptr(s_ptr.add(24)) }.to_string_lossy();
    let upper = s_str.to_uppercase();
    allocate_pace_string(&upper)
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
pub extern "C" fn paceIntToString(value: i64) -> *const c_char {
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
pub extern "C" fn paceFloatToString(value: f64) -> *const c_char {
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
pub extern "C" fn paceBoolToString(value: i64) -> *const c_char {
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

// ---------------------------------------------------------
// FFI Math Operations
// ---------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mathSqrt(x: f64) -> f64 {
    x.sqrt()
}

#[unsafe(no_mangle)]
pub extern "C" fn mathPow(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

#[unsafe(no_mangle)]
pub extern "C" fn mathAbs(x: f64) -> f64 {
    x.abs()
}

#[unsafe(no_mangle)]
pub extern "C" fn mathSin(x: f64) -> f64 {
    x.sin()
}

#[unsafe(no_mangle)]
pub extern "C" fn mathCos(x: f64) -> f64 {
    x.cos()
}

#[unsafe(no_mangle)]
pub extern "C" fn retainJsonValue(val: *mut u8) {
    if !val.is_null() {
        pace_retain(val);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn retainString(val: *mut u8) {
    if !val.is_null() {
        pace_retain(val);
    }
}

// ---------------------------------------------------------
// HTTP
// ---------------------------------------------------------

struct PaceHttpRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
}

struct PaceHttpResponse {
    status: u16,
    body: String,
    headers: Vec<(String, String)>,
}

#[unsafe(no_mangle)]
pub extern "C" fn httpClientCreateWithOptions(timeout_secs: i64, user_agent_ptr: *const c_char) -> *mut core::ffi::c_void {
    let mut builder = ureq::builder();
    
    if timeout_secs > 0 {
        builder = builder.timeout(std::time::Duration::from_secs(timeout_secs as u64));
    }
    
    if !user_agent_ptr.is_null() {
        let ua = unsafe { std::ffi::CStr::from_ptr(user_agent_ptr.add(24)).to_string_lossy().into_owned() };
        if !ua.is_empty() {
            builder = builder.user_agent(&ua);
        }
    }

    let agent = builder.build();
    Box::into_raw(Box::new(agent)) as *mut core::ffi::c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn httpClientFree(ptr: *mut core::ffi::c_void) {
    if !ptr.is_null() {
        unsafe {
            let _ = Box::from_raw(ptr as *mut ureq::Agent);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpRequestCreate(method_ptr: *const c_char, url_ptr: *const c_char) -> *mut core::ffi::c_void {
    unsafe {
        let method = if method_ptr.is_null() { "GET".to_string() } else { std::ffi::CStr::from_ptr(method_ptr.add(24)).to_string_lossy().into_owned() };
        let url = if url_ptr.is_null() { "".to_string() } else { std::ffi::CStr::from_ptr(url_ptr.add(24)).to_string_lossy().into_owned() };
        
        let req = Box::new(PaceHttpRequest {
            method,
            url,
            headers: Vec::new(),
            body: None,
        });
        Box::into_raw(req) as *mut core::ffi::c_void
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpRequestAddHeader(req_ptr: *mut core::ffi::c_void, key_ptr: *const c_char, val_ptr: *const c_char) {
    if req_ptr.is_null() || key_ptr.is_null() || val_ptr.is_null() { return; }
    unsafe {
        let req = &mut *(req_ptr as *mut PaceHttpRequest);
        let key = std::ffi::CStr::from_ptr(key_ptr.add(24)).to_string_lossy().into_owned();
        let val = std::ffi::CStr::from_ptr(val_ptr.add(24)).to_string_lossy().into_owned();
        req.headers.push((key, val));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpRequestSetBody(req_ptr: *mut core::ffi::c_void, body_ptr: *const c_char) {
    if req_ptr.is_null() { return; }
    unsafe {
        let req = &mut *(req_ptr as *mut PaceHttpRequest);
        let body = if body_ptr.is_null() { "".to_string() } else { std::ffi::CStr::from_ptr(body_ptr.add(24)).to_string_lossy().into_owned() };
        req.body = Some(body);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpRequestFree(ptr: *mut core::ffi::c_void) {
    if !ptr.is_null() {
        unsafe {
            let _ = Box::from_raw(ptr as *mut PaceHttpRequest);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpClientSend(agent_ptr: *mut core::ffi::c_void, req_ptr: *mut core::ffi::c_void) -> *mut core::ffi::c_void {
    if agent_ptr.is_null() || req_ptr.is_null() {
        return core::ptr::null_mut();
    }
    unsafe {
        let agent = &*(agent_ptr as *const ureq::Agent);
        let req = &*(req_ptr as *const PaceHttpRequest);
        
        let mut request = agent.request(&req.method, &req.url);
        for (k, v) in &req.headers {
            request = request.set(k, v);
        }
        
        let response_result = if let Some(body) = &req.body {
            request.send_string(body)
        } else {
            request.call()
        };
        
        let (status, body, headers) = match response_result {
            Ok(response) => {
                let status = response.status();
                let mut headers = Vec::new();
                for name in response.headers_names() {
                    if let Some(val) = response.header(&name) {
                        headers.push((name, val.to_string()));
                    }
                }
                let body = response.into_string().unwrap_or_default();
                (status, body, headers)
            }
            Err(ureq::Error::Status(code, response)) => {
                let mut headers = Vec::new();
                for name in response.headers_names() {
                    if let Some(val) = response.header(&name) {
                        headers.push((name, val.to_string()));
                    }
                }
                let body = response.into_string().unwrap_or_default();
                (code, body, headers)
            }
            Err(_) => {
                return core::ptr::null_mut();
            }
        };

        let pace_resp = Box::new(PaceHttpResponse { status, body, headers });
        Box::into_raw(pace_resp) as *mut core::ffi::c_void
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpResponseIsValid(ptr: *mut core::ffi::c_void) -> i64 {
    if ptr.is_null() { 0 } else { 1 }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpResponseGetStatus(ptr: *mut core::ffi::c_void) -> i64 {
    if ptr.is_null() { return 0; }
    unsafe {
        let response = &*(ptr as *const PaceHttpResponse);
        response.status as i64
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpResponseGetBody(ptr: *mut core::ffi::c_void) -> *const c_char {
    if ptr.is_null() { return allocate_pace_string("") as *const c_char; }
    unsafe {
        let response = &*(ptr as *const PaceHttpResponse);
        allocate_pace_string(&response.body) as *const c_char
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpResponseGetHeader(ptr: *mut core::ffi::c_void, key_ptr: *const c_char) -> *const c_char {
    if ptr.is_null() || key_ptr.is_null() { return std::ptr::null(); }
    unsafe {
        let response = &*(ptr as *const PaceHttpResponse);
        let search_key = std::ffi::CStr::from_ptr(key_ptr.add(24)).to_string_lossy().into_owned().to_lowercase();
        
        for (k, v) in &response.headers {
            if k.to_lowercase() == search_key {
                return allocate_pace_string(v) as *const c_char;
            }
        }
        std::ptr::null()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpResponseFree(ptr: *mut core::ffi::c_void) {
    if ptr.is_null() { return; }
    unsafe {
        let _ = Box::from_raw(ptr as *mut PaceHttpResponse);
    }
}

// ---------------------------------------------------------
// HTTP SERVER
// ---------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn httpServerBind(addr_ptr: *const c_char) -> *mut core::ffi::c_void {
    if addr_ptr.is_null() {
        return core::ptr::null_mut();
    }
    unsafe {
        let addr = std::ffi::CStr::from_ptr(addr_ptr.add(24)).to_string_lossy();
        match tiny_http::Server::http(addr.as_ref()) {
            Ok(server) => Box::into_raw(Box::new(server)) as *mut core::ffi::c_void,
            Err(_) => core::ptr::null_mut(),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpServerAccept(server_ptr: *mut core::ffi::c_void) -> *mut core::ffi::c_void {
    if server_ptr.is_null() {
        return core::ptr::null_mut();
    }
    unsafe {
        let server = &*(server_ptr as *const tiny_http::Server);
        match server.recv() {
            Ok(request) => Box::into_raw(Box::new(request)) as *mut core::ffi::c_void,
            Err(_) => core::ptr::null_mut(),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpServerFree(server_ptr: *mut core::ffi::c_void) {
    if server_ptr.is_null() {
        return;
    }
    unsafe {
        let _ = Box::from_raw(server_ptr as *mut tiny_http::Server);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpRequestGetPath(req_ptr: *mut core::ffi::c_void) -> *const c_char {
    if req_ptr.is_null() {
        return allocate_pace_string("") as *const c_char;
    }
    unsafe {
        let request = &*(req_ptr as *const tiny_http::Request);
        allocate_pace_string(request.url()) as *const c_char
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpRequestMethod(req_ptr: *mut core::ffi::c_void) -> *const c_char {
    if req_ptr.is_null() {
        return allocate_pace_string("") as *const c_char;
    }
    unsafe {
        let request = &*(req_ptr as *const tiny_http::Request);
        allocate_pace_string(request.method().as_str()) as *const c_char
    }
}

struct PaceHttpServerResponse {
    status: u16,
    body: String,
    headers: Vec<(String, String)>,
}

#[unsafe(no_mangle)]
pub extern "C" fn httpServerResponseCreate(status: i64, body_ptr: *const c_char) -> *mut core::ffi::c_void {
    let body = if body_ptr.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(body_ptr.add(24)).to_string_lossy().into_owned() }
    };
    let resp = Box::new(PaceHttpServerResponse {
        status: status as u16,
        body,
        headers: Vec::new(),
    });
    Box::into_raw(resp) as *mut core::ffi::c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn httpServerResponseAddHeader(resp_ptr: *mut core::ffi::c_void, key_ptr: *const c_char, val_ptr: *const c_char) {
    if resp_ptr.is_null() || key_ptr.is_null() || val_ptr.is_null() { return; }
    unsafe {
        let resp = &mut *(resp_ptr as *mut PaceHttpServerResponse);
        let key = std::ffi::CStr::from_ptr(key_ptr.add(24)).to_string_lossy().into_owned();
        let val = std::ffi::CStr::from_ptr(val_ptr.add(24)).to_string_lossy().into_owned();
        resp.headers.push((key, val));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpRequestRespond(req_ptr: *mut core::ffi::c_void, resp_ptr: *mut core::ffi::c_void) {
    if req_ptr.is_null() || resp_ptr.is_null() { return; }
    unsafe {
        let request = *Box::from_raw(req_ptr as *mut tiny_http::Request);
        let resp_data = *Box::from_raw(resp_ptr as *mut PaceHttpServerResponse);
        
        let mut response = tiny_http::Response::from_string(resp_data.body)
            .with_status_code(resp_data.status);
            
        for (k, v) in resp_data.headers {
            if let Ok(header) = tiny_http::Header::from_bytes(k.as_bytes(), v.as_bytes()) {
                response.add_header(header);
            }
        }
            
        let _ = request.respond(response);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpRequestGetHeader(req_ptr: *mut core::ffi::c_void, key_ptr: *const c_char) -> *const c_char {
    if req_ptr.is_null() || key_ptr.is_null() { return std::ptr::null(); }
    unsafe {
        let request = &*(req_ptr as *const tiny_http::Request);
        let search_key = std::ffi::CStr::from_ptr(key_ptr.add(24)).to_string_lossy().into_owned().to_lowercase();
        
        for header in request.headers() {
            if header.field.as_str().to_string().to_lowercase() == search_key {
                return allocate_pace_string(header.value.as_str()) as *const c_char;
            }
        }
        std::ptr::null()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpRequestGetBody(req_ptr: *mut core::ffi::c_void) -> *const c_char {
    if req_ptr.is_null() { return allocate_pace_string("") as *const c_char; }
    unsafe {
        let request = &mut *(req_ptr as *mut tiny_http::Request);
        let mut body = String::new();
        if let Err(_) = std::io::Read::read_to_string(request.as_reader(), &mut body) {
            return allocate_pace_string("") as *const c_char;
        }
        allocate_pace_string(&body) as *const c_char
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn httpRequestGetQuery(req_ptr: *mut core::ffi::c_void, key_ptr: *const c_char) -> *const c_char {
    if req_ptr.is_null() || key_ptr.is_null() { return std::ptr::null(); }
    unsafe {
        let request = &*(req_ptr as *const tiny_http::Request);
        let search_key = std::ffi::CStr::from_ptr(key_ptr.add(24)).to_string_lossy();
        
        let url = request.url();
        if let Some(query_str) = url.splitn(2, '?').nth(1) {
            for pair in query_str.split('&') {
                let mut parts = pair.splitn(2, '=');
                let k = parts.next().unwrap_or("");
                let v = parts.next().unwrap_or("");
                if k == search_key {
                    // Quick decode for simple cases (e.g. + to space). Note: Full URL decode isn't strictly required here for basic support but it's helpful
                    let decoded_v = v.replace("+", " ");
                    // Simple percent decoding could be added later, keep it basic for now
                    return allocate_pace_string(&decoded_v) as *const c_char;
                }
            }
        }
        std::ptr::null()
    }
}
