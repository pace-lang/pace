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
pub extern "C" fn __pace_concat_strings(
    a: *const std::ffi::c_char,
    b: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    unsafe {
        let str_a = if a.is_null() {
            ""
        } else {
            std::ffi::CStr::from_ptr(a).to_str().unwrap_or("")
        };
        let str_b = if b.is_null() {
            ""
        } else {
            std::ffi::CStr::from_ptr(b).to_str().unwrap_or("")
        };
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
pub extern "C" fn __pace_free_string(ptr: *mut std::ffi::c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = std::ffi::CString::from_raw(ptr);
        }
    }
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
pub extern "C" fn __pace_hash(val: i64) -> i64 {
    // A simple integer hash function (e.g. FNV-1a or splitmix64 style)
    let mut x = val as u64;
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58476d1ce4e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d049bb133111eb);
    x ^= x >> 31;
    x as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_noop(_obj: *mut u8) {
    // No-op for primitive retain/release
    std::hint::black_box(_obj);
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

#[unsafe(no_mangle)]
pub extern "C" fn __pace_sb_new() -> *mut String {
    Box::into_raw(Box::new(String::with_capacity(32)))
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_sb_append(ptr: *mut String, s: *const std::ffi::c_char) {
    if ptr.is_null() || s.is_null() {
        return;
    }
    unsafe {
        let sb = &mut *ptr;
        let c_str = std::ffi::CStr::from_ptr(s);
        if let Ok(s_str) = c_str.to_str() {
            sb.push_str(s_str);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_sb_build(ptr: *mut String) -> *mut std::ffi::c_char {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let sb = &*ptr;
        let c_string = std::ffi::CString::new(sb.as_str()).unwrap();
        c_string.into_raw()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_sb_free(ptr: *mut String) {
    if !ptr.is_null() {
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }
}

thread_local! {
    static LAST_ERROR_MESSAGE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn set_last_error(msg: &str) {
    LAST_ERROR_MESSAGE.with(|e| {
        *e.borrow_mut() = msg.to_string();
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_get_last_error() -> *mut std::ffi::c_char {
    LAST_ERROR_MESSAGE.with(|e| {
        let mut msg = e.borrow_mut();
        let c_string = std::ffi::CString::new(msg.as_str())
            .unwrap_or_else(|_| std::ffi::CString::new("").unwrap());
        *msg = String::new(); // consume
        c_string.into_raw()
    })
}

// String FFI Functions
#[unsafe(no_mangle)]
pub extern "C" fn __pace_string_split(
    s: *const std::ffi::c_char,
    delim: *const std::ffi::c_char,
) -> *mut i64 {
    if s.is_null() || delim.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let s_str = std::ffi::CStr::from_ptr(s).to_string_lossy();
        let delim_str = std::ffi::CStr::from_ptr(delim).to_string_lossy();

        let parts: Vec<&str> = s_str.split(delim_str.as_ref()).collect();
        let len = parts.len();

        let mut arr: Vec<i64> = Vec::with_capacity(len + 1);
        arr.push(len as i64);
        for part in parts {
            let c_str = std::ffi::CString::new(part)
                .unwrap_or_else(|_| std::ffi::CString::new("").unwrap());
            arr.push(c_str.into_raw() as i64);
        }

        let ptr = arr.as_mut_ptr();
        std::mem::forget(arr);
        ptr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_string_replace(
    s: *const std::ffi::c_char,
    old: *const std::ffi::c_char,
    new: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    if s.is_null() || old.is_null() || new.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let s_str = std::ffi::CStr::from_ptr(s).to_string_lossy();
        let old_str = std::ffi::CStr::from_ptr(old).to_string_lossy();
        let new_str = std::ffi::CStr::from_ptr(new).to_string_lossy();

        let replaced = s_str.replace(old_str.as_ref(), new_str.as_ref());
        std::ffi::CString::new(replaced)
            .unwrap_or_else(|_| std::ffi::CString::new("").unwrap())
            .into_raw()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_string_substring(
    s: *const std::ffi::c_char,
    start: i64,
    end: i64,
) -> *mut std::ffi::c_char {
    if s.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let s_str = std::ffi::CStr::from_ptr(s).to_string_lossy();
        let len = s_str.len() as i64;
        let start = start.max(0).min(len) as usize;
        let end = end.max(0).min(len) as usize;

        let sub = if start <= end { &s_str[start..end] } else { "" };
        std::ffi::CString::new(sub)
            .unwrap_or_else(|_| std::ffi::CString::new("").unwrap())
            .into_raw()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_string_trim(s: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    if s.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let s_str = std::ffi::CStr::from_ptr(s).to_string_lossy();
        std::ffi::CString::new(s_str.trim())
            .unwrap_or_else(|_| std::ffi::CString::new("").unwrap())
            .into_raw()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_string_index_of(
    s: *const std::ffi::c_char,
    search: *const std::ffi::c_char,
) -> i64 {
    if s.is_null() || search.is_null() {
        return -1;
    }
    unsafe {
        let s_str = std::ffi::CStr::from_ptr(s).to_string_lossy();
        let search_str = std::ffi::CStr::from_ptr(search).to_string_lossy();
        if let Some(idx) = s_str.find(search_str.as_ref()) {
            idx as i64
        } else {
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_string_starts_with(
    s: *const std::ffi::c_char,
    search: *const std::ffi::c_char,
) -> i64 {
    if s.is_null() || search.is_null() {
        return 0;
    }
    unsafe {
        let s_str = std::ffi::CStr::from_ptr(s).to_string_lossy();
        let search_str = std::ffi::CStr::from_ptr(search).to_string_lossy();
        if s_str.starts_with(search_str.as_ref()) {
            1
        } else {
            0
        }
    }
}

// ==========================================
// FS RUNTIME (File System)
// ==========================================

#[unsafe(no_mangle)]
pub extern "C" fn __pace_fs_write(
    path: *const std::ffi::c_char,
    content: *const std::ffi::c_char,
) -> i64 {
    if path.is_null() || content.is_null() {
        return 0;
    }
    unsafe {
        let p_str = std::ffi::CStr::from_ptr(path)
            .to_string_lossy()
            .into_owned();
        let c_str = std::ffi::CStr::from_ptr(content)
            .to_string_lossy()
            .into_owned();
        match std::fs::write(p_str, c_str) {
            Ok(_) => 1,
            Err(e) => {
                set_last_error(&e.to_string());
                0
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_fs_exists(path: *const std::ffi::c_char) -> i64 {
    if path.is_null() {
        return 0;
    }
    unsafe {
        let p_str = std::ffi::CStr::from_ptr(path)
            .to_string_lossy()
            .into_owned();
        match std::fs::metadata(p_str) {
            Ok(m) => {
                if m.is_file() {
                    1
                } else {
                    0
                }
            }
            Err(e) => {
                set_last_error(&e.to_string());
                0
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_fs_read(path: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    if path.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let p_str = std::ffi::CStr::from_ptr(path)
            .to_string_lossy()
            .into_owned();
        match std::fs::read_to_string(p_str) {
            Ok(content) => std::ffi::CString::new(content)
                .unwrap_or_else(|_| std::ffi::CString::new("").unwrap())
                .into_raw(),
            Err(e) => {
                set_last_error(&e.to_string());
                std::ptr::null_mut()
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_fs_delete(path: *const std::ffi::c_char) -> i64 {
    if path.is_null() {
        return 0;
    }
    unsafe {
        let p_str = std::ffi::CStr::from_ptr(path)
            .to_string_lossy()
            .into_owned();
        match std::fs::remove_file(p_str) {
            Ok(_) => 1,
            Err(e) => {
                set_last_error(&e.to_string());
                0
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_fs_mkdir(path: *const std::ffi::c_char) -> i64 {
    if path.is_null() {
        return 0;
    }
    unsafe {
        let p_str = std::ffi::CStr::from_ptr(path)
            .to_string_lossy()
            .into_owned();
        match std::fs::create_dir_all(p_str) {
            Ok(_) => 1,
            Err(e) => {
                set_last_error(&e.to_string());
                0
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_fs_dir_exists(path: *const std::ffi::c_char) -> i64 {
    if path.is_null() {
        return 0;
    }
    unsafe {
        let p_str = std::ffi::CStr::from_ptr(path)
            .to_string_lossy()
            .into_owned();
        match std::fs::metadata(p_str) {
            Ok(m) => {
                if m.is_dir() {
                    1
                } else {
                    0
                }
            }
            Err(e) => {
                set_last_error(&e.to_string());
                0
            }
        }
    }
}

// ==========================================
// OS AND PROCESS RUNTIME
// ==========================================

#[unsafe(no_mangle)]
pub extern "C" fn __pace_os_getenv(key: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    if key.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let k_str = std::ffi::CStr::from_ptr(key).to_string_lossy().into_owned();
        match std::env::var(k_str) {
            Ok(val) => std::ffi::CString::new(val)
                .unwrap_or_else(|_| std::ffi::CString::new("").unwrap())
                .into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_os_name() -> *mut std::ffi::c_char {
    std::ffi::CString::new(std::env::consts::OS)
        .unwrap()
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_process_run(command: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    if command.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let cmd_str = std::ffi::CStr::from_ptr(command)
            .to_string_lossy()
            .into_owned();

        let output = if cfg!(target_os = "windows") {
            std::process::Command::new("cmd")
                .args(["/C", &cmd_str])
                .output()
        } else {
            std::process::Command::new("sh")
                .args(["-c", &cmd_str])
                .output()
        };

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
                std::ffi::CString::new(stdout)
                    .unwrap_or_else(|_| std::ffi::CString::new("").unwrap())
                    .into_raw()
            }
            Err(e) => {
                set_last_error(&e.to_string());
                std::ptr::null_mut()
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_process_exit(code: i64) -> ! {
    std::process::exit(code as i32);
}

// ==========================================
// HTTP RUNTIME (Network)
// ==========================================

#[unsafe(no_mangle)]
pub extern "C" fn __pace_http_get(url: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    if url.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let u_str = std::ffi::CStr::from_ptr(url).to_string_lossy().into_owned();
        match ureq::get(&u_str).call() {
            Ok(response) => {
                let mut content = String::new();
                use std::io::Read;
                let _ = response
                    .into_body()
                    .into_reader()
                    .read_to_string(&mut content);
                std::ffi::CString::new(content)
                    .unwrap_or_else(|_| std::ffi::CString::new("").unwrap())
                    .into_raw()
            }
            Err(e) => {
                set_last_error(&e.to_string());
                std::ptr::null_mut()
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_http_post(
    url: *const std::ffi::c_char,
    body: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    if url.is_null() || body.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let u_str = std::ffi::CStr::from_ptr(url).to_string_lossy().into_owned();
        let b_str = std::ffi::CStr::from_ptr(body)
            .to_string_lossy()
            .into_owned();
        match ureq::post(&u_str).send(&b_str) {
            Ok(response) => {
                let mut content = String::new();
                use std::io::Read;
                let _ = response
                    .into_body()
                    .into_reader()
                    .read_to_string(&mut content);
                std::ffi::CString::new(content)
                    .unwrap_or_else(|_| std::ffi::CString::new("").unwrap())
                    .into_raw()
            }
            Err(e) => {
                set_last_error(&e.to_string());
                std::ptr::null_mut()
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_http_put(
    url: *const std::ffi::c_char,
    body: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    if url.is_null() || body.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let u_str = std::ffi::CStr::from_ptr(url).to_string_lossy().into_owned();
        let b_str = std::ffi::CStr::from_ptr(body)
            .to_string_lossy()
            .into_owned();
        match ureq::put(&u_str).send(&b_str) {
            Ok(response) => {
                let mut content = String::new();
                use std::io::Read;
                let _ = response
                    .into_body()
                    .into_reader()
                    .read_to_string(&mut content);
                std::ffi::CString::new(content)
                    .unwrap_or_else(|_| std::ffi::CString::new("").unwrap())
                    .into_raw()
            }
            Err(e) => {
                set_last_error(&e.to_string());
                std::ptr::null_mut()
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_http_delete(url: *const std::ffi::c_char) -> *mut std::ffi::c_char {
    if url.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let u_str = std::ffi::CStr::from_ptr(url).to_string_lossy().into_owned();
        match ureq::delete(&u_str).call() {
            Ok(response) => {
                let mut content = String::new();
                use std::io::Read;
                let _ = response
                    .into_body()
                    .into_reader()
                    .read_to_string(&mut content);
                std::ffi::CString::new(content)
                    .unwrap_or_else(|_| std::ffi::CString::new("").unwrap())
                    .into_raw()
            }
            Err(e) => {
                set_last_error(&e.to_string());
                std::ptr::null_mut()
            }
        }
    }
}

// ==========================================
// ACTOR RUNTIME (Mailbox, ThreadPool, Promise)
// ==========================================

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread;

pub struct Task {
    func: extern "C" fn(i64) -> i64,
    arg: i64,
    promise: *mut Promise,
}

pub struct Mailbox {
    queue: Mutex<VecDeque<Task>>,
    is_scheduled: AtomicBool,
}

struct ThreadPool {
    actor_sender: mpsc::Sender<usize>,
}

static THREAD_POOL: OnceLock<ThreadPool> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "C" fn __pace_init_runtime(num_threads: usize) {
    THREAD_POOL.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<usize>();
        let receiver = Arc::new(Mutex::new(receiver));

        let num_threads = if num_threads == 0 { 4 } else { num_threads };

        for _ in 0..num_threads {
            let rx = Arc::clone(&receiver);
            thread::spawn(move || {
                loop {
                    let mb_ptr = {
                        let lock = rx.lock().unwrap();
                        lock.recv()
                    };
                    match mb_ptr {
                        Ok(ptr) => {
                            let mailbox = unsafe { &*(ptr as *mut Mailbox) };
                            loop {
                                let task = {
                                    let mut q = mailbox.queue.lock().unwrap();
                                    q.pop_front()
                                };
                                match task {
                                    Some(t) => {
                                        let result = (t.func)(t.arg);
                                        if !t.promise.is_null() {
                                            __pace_promise_resolve(t.promise, result);
                                        }
                                    }
                                    None => {
                                        mailbox.is_scheduled.store(false, Ordering::SeqCst);
                                        // Double-check queue to avoid race condition
                                        let q = mailbox.queue.lock().unwrap();
                                        if !q.is_empty()
                                            && !mailbox.is_scheduled.swap(true, Ordering::SeqCst)
                                        {
                                            // Keep processing since a message arrived right after we stored false
                                            continue;
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                        Err(_) => break, // Channel closed
                    }
                }
            });
        }

        ThreadPool {
            actor_sender: sender,
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_mailbox_create() -> *mut Mailbox {
    let mb = Box::new(Mailbox {
        queue: Mutex::new(VecDeque::new()),
        is_scheduled: AtomicBool::new(false),
    });
    Box::into_raw(mb)
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_mailbox_send(
    mb: *mut Mailbox,
    func: extern "C" fn(i64) -> i64,
    arg: i64,
    promise: *mut Promise,
) {
    if mb.is_null() {
        return;
    }
    let mailbox = unsafe { &*mb };

    mailbox
        .queue
        .lock()
        .unwrap()
        .push_back(Task { func, arg, promise });

    if !mailbox.is_scheduled.swap(true, Ordering::SeqCst) {
        if let Some(pool) = THREAD_POOL.get() {
            let _ = pool.actor_sender.send(mb as usize);
        } else {
            __pace_init_runtime(4);
            if let Some(pool) = THREAD_POOL.get() {
                let _ = pool.actor_sender.send(mb as usize);
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_mailbox_destroy(mb: *mut Mailbox) {
    if !mb.is_null() {
        unsafe {
            let _ = Box::from_raw(mb);
        }
    }
}

pub struct Promise {
    result: Mutex<Option<i64>>,
    condvar: std::sync::Condvar,
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_promise_create() -> *mut Promise {
    let p = Box::new(Promise {
        result: Mutex::new(None),
        condvar: std::sync::Condvar::new(),
    });
    Box::into_raw(p)
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_promise_resolve(p: *mut Promise, val: i64) {
    if p.is_null() {
        return;
    }
    let promise = unsafe { &*p };
    let mut result = promise.result.lock().unwrap();
    *result = Some(val);
    promise.condvar.notify_all();
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_promise_await(p: *mut Promise) -> i64 {
    if p.is_null() {
        return 0;
    }
    let promise = unsafe { &*p };
    let mut result = promise.result.lock().unwrap();
    while result.is_none() {
        result = promise.condvar.wait(result).unwrap();
    }
    result.unwrap()
}

#[unsafe(no_mangle)]
pub extern "C" fn __pace_promise_destroy(p: *mut Promise) {
    if !p.is_null() {
        unsafe {
            let _ = Box::from_raw(p);
        }
    }
}
