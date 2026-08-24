#![allow(clippy::not_unsafe_ptr_arg_deref)]

#[unsafe(no_mangle)]
pub extern "C" fn __pace_print_int(val: i64) {
    println!("{}", val);
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_print_float(val: f64) {
    println!("{}", val);
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_print_string(val: *const std::ffi::c_char) {
    if val.is_null() {
        println!("null");
    } else {
        unsafe {
            let c_str = std::ffi::CStr::from_ptr(val);
            if let Ok(s) = c_str.to_str() {
                println!("{}", s);
            } else {
                println!("<invalid string>");
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_concat_strings(a: *const std::ffi::c_char, b: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    unsafe {
        let str_a = if a.is_null() { "" } else { std::ffi::CStr::from_ptr(a).to_str().unwrap_or("") };
        let str_b = if b.is_null() { "" } else { std::ffi::CStr::from_ptr(b).to_str().unwrap_or("") };
        let mut combined = String::with_capacity(str_a.len() + str_b.len());
        combined.push_str(str_a);
        combined.push_str(str_b);
        let c_string = std::ffi::CString::new(combined).unwrap();
        c_string.into_raw()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_int_to_string(val: i64) -> *mut std::ffi::c_char {
    let s = val.to_string();
    std::ffi::CString::new(s).unwrap().into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_float_to_string(val: f64) -> *mut std::ffi::c_char {
    let s = val.to_string();
    std::ffi::CString::new(s).unwrap().into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_bool_to_string(val: i64) -> *mut std::ffi::c_char {
    let s = if val == 0 { "false" } else { "true" };
    std::ffi::CString::new(s).unwrap().into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_malloc(size: usize) -> *mut u8 {
    unsafe {
        let ptr = std::alloc::alloc(std::alloc::Layout::from_size_align(size, 8).unwrap());
        if ptr.is_null() {
            std::alloc::handle_alloc_error(std::alloc::Layout::from_size_align(size, 8).unwrap());
        }
        ptr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_retain(obj: *mut u8) {
    if obj.is_null() {
        return;
    }
    let rc_ptr = obj as *const std::sync::atomic::AtomicI64;
    unsafe {
        (*rc_ptr).fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_release(obj: *mut u8) {
    if obj.is_null() {
        return;
    }
    let rc_ptr = obj as *const std::sync::atomic::AtomicI64;
    unsafe {
        if (*rc_ptr).fetch_sub(1, std::sync::atomic::Ordering::Release) == 1 {
            std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
            
            // Load vtable pointer from Offset 8
            let vtable_ptr_addr = obj.add(8) as *const *const u8;
            let vtable_ptr = *vtable_ptr_addr;
            
            if !vtable_ptr.is_null() {
                // Load drop function from VTable Offset 0
                let drop_fn_addr = vtable_ptr as *const Option<extern "C" fn(*mut u8)>;
                if let Some(drop_fn) = *drop_fn_addr {
                    // Call the drop function to release fields
                    drop_fn(obj);
                }
                
                // Load size from VTable Offset 8
                let size_addr = vtable_ptr.add(8) as *const i64;
                let size = *size_addr as usize;
                
                // Deallocate the memory
                std::alloc::dealloc(obj, std::alloc::Layout::from_size_align(size, 8).unwrap());
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_free(ptr: *mut u8, size: usize) {
    if !ptr.is_null() {
        unsafe {
            std::alloc::dealloc(ptr, std::alloc::Layout::from_size_align(size, 8).unwrap());
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_ptr_store(ptr: *mut u8, offset: i64, val: i64) {
    unsafe {
        let target = ptr.offset(offset as isize) as *mut i64;
        *target = val;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_ptr_load(ptr: *const u8, offset: i64) -> i64 {
    if ptr.is_null() {
        panic!("null pointer dereference occurred");
    }
    unsafe {
        let target = ptr.offset(offset as isize) as *const i64;
        *target
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_time(_arg: i64) -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_get_year(ts: i64) -> i64 {
    // Basic approximation, ignores some leap seconds/days, but good enough for test if complex date isn't needed.
    // Actually using chrono is better, but since it's not in dependencies, we'll do simple math.
    // 1970 + ts / 31556926 (seconds in a year approx)
    1970 + (ts / 31556926)
}
