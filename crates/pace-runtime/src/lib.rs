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
pub extern "C" fn __pace_malloc(size: usize) -> *mut u8 {
    unsafe {
        let ptr = std::alloc::alloc(std::alloc::Layout::from_size_align(size, 8).unwrap());
        if ptr.is_null() {
            std::alloc::handle_alloc_error(std::alloc::Layout::from_size_align(size, 8).unwrap());
        }
        ptr
    }
}
