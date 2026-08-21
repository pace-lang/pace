#![allow(clippy::missing_safety_doc)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
pub mod async_rt;
pub use async_rt::*;
pub mod actor;
pub mod net;
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
pub extern "C" fn paceSleep(ms: i64) {
    if ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }
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
        // Size is not stored in the header, wait... how do we know the size?
        // Actually, we DO store the size, no we don't.
        // Wait, std::alloc::dealloc REQUIRES the Layout size!
        // We can't use std::alloc::dealloc without the size.
        // I will revert pace_alloc to use libc::calloc/malloc!
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
pub extern "C" fn stringFindNumberEnd(s: *const c_char, start: i64) -> i64 {
    if s.is_null() || start < 0 { return -1; }
    unsafe {
        let mut i = start as usize;
        let str_ptr = s.add(24);
        loop {
            let b = *(str_ptr.add(i) as *const u8);
            if b == 0 { break; }
            let is_num_char = match b {
                b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-' => true,
                _ => false,
            };
            if !is_num_char {
                return i as i64;
            }
            i += 1;
        }
        i as i64
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn stringMatchKeywordAt(s: *const c_char, start: i64, keyword: *const c_char) -> i64 {
    if s.is_null() || keyword.is_null() || start < 0 { return 0; }
    unsafe {
        let str_ptr = s.add(24) as *const u8;
        let key_ptr = keyword.add(24) as *const u8;
        
        let mut i = 0;
        loop {
            let kb = *(key_ptr.add(i));
            if kb == 0 { return 1; } // Matched completely
            let sb = *(str_ptr.add(start as usize + i));
            if sb == 0 || sb != kb { return 0; } // Mismatch or end of string
            i += 1;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn stringSubstringToInt(s: *const c_char, start: i64, end: i64) -> i64 {
    if s.is_null() || start < 0 || end <= start { return 0; }
    unsafe {
        let str_ptr = s.add(24) as *const u8;
        let slice = std::slice::from_raw_parts(str_ptr.add(start as usize), (end - start) as usize);
        if let Ok(s_str) = std::str::from_utf8(slice) {
            if let Ok(val) = s_str.parse::<i64>() {
                return val;
            }
        }
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn stringSubstringToFloat(s: *const c_char, start: i64, end: i64) -> f64 {
    if s.is_null() || start < 0 || end <= start { return 0.0; }
    unsafe {
        let str_ptr = s.add(24) as *const u8;
        let slice = std::slice::from_raw_parts(str_ptr.add(start as usize), (end - start) as usize);
        if let Ok(s_str) = std::str::from_utf8(slice) {
            if let Ok(val) = s_str.parse::<f64>() {
                return val;
            }
        }
        0.0
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
pub extern "C" fn fileIsValid(ptr: *mut core::ffi::c_void) -> i64 {
    if ptr.is_null() { 0 } else { 1 }
}

#[unsafe(no_mangle)]
pub extern "C" fn fileReadAll(ptr: *mut core::ffi::c_void) -> *const c_char {
    if ptr.is_null() {
        return allocate_pace_string("");
    }
    let file = unsafe { &mut *(ptr as *mut std::fs::File) };
    use std::io::Read;
    let mut contents = String::new();
    if file.read_to_string(&mut contents).is_ok() {
        return allocate_pace_string(&contents);
    }
    allocate_pace_string("")
}

#[unsafe(no_mangle)]
pub extern "C" fn fileClose(ptr: *mut core::ffi::c_void) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr as *mut std::fs::File)) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fileWrite(ptr: *mut core::ffi::c_void, data_ptr: *const c_char) {
    if ptr.is_null() || data_ptr.is_null() { return; }
    let file = unsafe { &mut *(ptr as *mut std::fs::File) };
    let data = unsafe { std::ffi::CStr::from_ptr(data_ptr.add(24)) }.to_bytes();
    use std::io::Write;
    let _ = file.write_all(data);
}

#[unsafe(no_mangle)]
pub extern "C" fn fileOpen(path_ptr: *const c_char, mode_ptr: *const c_char) -> *mut core::ffi::c_void {
    if path_ptr.is_null() || mode_ptr.is_null() { return std::ptr::null_mut(); }
    let path_str = unsafe { std::ffi::CStr::from_ptr(path_ptr.add(24)) }.to_string_lossy();
    let mode_str = unsafe { std::ffi::CStr::from_ptr(mode_ptr.add(24)) }.to_string_lossy();
    
    let mut options = std::fs::OpenOptions::new();
    if mode_str == "r" {
        options.read(true);
    } else if mode_str == "w" {
        options.write(true).create(true).truncate(true);
    } else if mode_str == "a" {
        options.write(true).create(true).append(true);
    } else if mode_str == "rw" {
        options.read(true).write(true).create(true);
    } else {
        options.read(true).write(true).create(true); // default
    }
    
    if let Ok(file) = options.open(path_str.as_ref()) {
        return Box::into_raw(Box::new(file)) as *mut core::ffi::c_void;
    }
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
pub extern "C" fn mathSqrt(x: f64) -> f64 { x.sqrt() }

#[unsafe(no_mangle)]
pub extern "C" fn mathPow(base: f64, exp: f64) -> f64 { base.powf(exp) }

#[unsafe(no_mangle)]
pub extern "C" fn mathSin(x: f64) -> f64 { x.sin() }

#[unsafe(no_mangle)]
pub extern "C" fn mathCos(x: f64) -> f64 { x.cos() }

#[unsafe(no_mangle)]
pub extern "C" fn mathCeil(x: f64) -> f64 { x.ceil() }

#[unsafe(no_mangle)]
pub extern "C" fn mathFloor(x: f64) -> f64 { x.floor() }

#[unsafe(no_mangle)]
pub extern "C" fn mathCbrt(x: f64) -> f64 { x.cbrt() }

#[unsafe(no_mangle)]
pub extern "C" fn mathExp(x: f64) -> f64 { x.exp() }

#[unsafe(no_mangle)]
pub extern "C" fn mathLog(x: f64) -> f64 { x.ln() }

#[unsafe(no_mangle)]
pub extern "C" fn mathTan(x: f64) -> f64 { x.tan() }

#[unsafe(no_mangle)]
pub extern "C" fn mathAsin(x: f64) -> f64 { x.asin() }

#[unsafe(no_mangle)]
pub extern "C" fn mathAcos(x: f64) -> f64 { x.acos() }

#[unsafe(no_mangle)]
pub extern "C" fn mathAtan(x: f64) -> f64 { x.atan() }

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



#[unsafe(no_mangle)]
pub extern "C" fn timeNow() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()
}

#[unsafe(no_mangle)]
pub extern "C" fn timeSleep(ms: f64) {
    if ms > 0.0 {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }
}
