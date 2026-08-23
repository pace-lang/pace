#[unsafe(no_mangle)]
pub extern "C" fn __pace_print_int(val: i64) {
    println!("pace says: {}", val);
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
