use std::ffi::CStr;
use std::os::raw::c_char;
use crate::{pace_alloc, pace_release};
use std::ptr;

// Pace string header size is 24 bytes (strong count + weak count + metadata)
const PACE_OBJECT_HEADER_SIZE: usize = 24;

/// Allocates a new empty StringBuilder (a Vec<u8> wrapped in a pointer)
#[unsafe(no_mangle)]
pub extern "C" fn paceStringBuilderNew() -> *mut u8 {
    let vec: Vec<u8> = Vec::with_capacity(1024);
    let vec_ptr = Box::into_raw(Box::new(vec));
    vec_ptr as *mut u8
}

/// Appends a Pace string to the builder
#[unsafe(no_mangle)]
pub extern "C" fn paceStringBuilderAppend(builder: *mut u8, s_ptr: *const c_char) {
    if builder.is_null() || s_ptr.is_null() {
        return;
    }
    
    // Extract the Vec<u8> from the raw pointer
    let mut vec = unsafe { Box::from_raw(builder as *mut Vec<u8>) };
    
    // Extract bytes from Pace String (which has a 24 byte header)
    let s_bytes = unsafe { CStr::from_ptr(s_ptr.add(PACE_OBJECT_HEADER_SIZE)) }.to_bytes();
    
    // Append to the vector
    vec.extend_from_slice(s_bytes);
    
    // Leak the box so it stays alive for the next call
    let _ = Box::into_raw(vec);
}

/// Builds a Pace string from the builder
#[unsafe(no_mangle)]
pub extern "C" fn paceStringBuilderToString(builder: *mut u8) -> *const c_char {
    if builder.is_null() {
        return ptr::null();
    }
    
    // Borrow the Vec<u8> to build the string
    let vec = unsafe { &*(builder as *const Vec<u8>) };
    
    let total_len = vec.len();
    
    // Allocate space for the combined string + null terminator + 24 byte header
    // Use -2 as metadata pointer to signify a string (as done in stringConcat)
    let new_ptr = pace_alloc((PACE_OBJECT_HEADER_SIZE + total_len + 1) as i64, (-2isize) as *const ());
    if new_ptr.is_null() {
        println!("Pace Runtime Error: Out of memory in StringBuilder");
        std::process::exit(1);
    }
    
    // Copy bytes to payload area (offset 24)
    unsafe {
        let payload_ptr = new_ptr.add(PACE_OBJECT_HEADER_SIZE);
        std::ptr::copy_nonoverlapping(vec.as_ptr(), payload_ptr, total_len);
        // null terminator already set by calloc inside pace_alloc
    }
    
    new_ptr as *const c_char
}

/// Drops the StringBuilder and its memory
#[unsafe(no_mangle)]
pub extern "C" fn paceStringBuilderDrop(builder: *mut u8) {
    if builder.is_null() {
        return;
    }
    // Reconstruct the Box and let it drop naturally
    let _ = unsafe { Box::from_raw(builder as *mut Vec<u8>) };
}
