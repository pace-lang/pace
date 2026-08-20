use std::ffi::{CStr, c_char, c_void};
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::sync::Arc;
use rustls::{ClientConfig, ClientConnection, StreamOwned, RootCertStore};
use rustls_pki_types::ServerName;

pub enum PaceStream {
    Tcp(TcpStream),
    Tls(StreamOwned<ClientConnection, TcpStream>),
}

impl Read for PaceStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            PaceStream::Tcp(s) => s.read(buf),
            PaceStream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for PaceStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            PaceStream::Tcp(s) => s.write(buf),
            PaceStream::Tls(s) => s.write(buf),
        }
    }
    
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            PaceStream::Tcp(s) => s.flush(),
            PaceStream::Tls(s) => s.flush(),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn paceTcpConnect(addr: *const c_char) -> *mut c_void {
    if addr.is_null() {
        return std::ptr::null_mut();
    }
    let addr_str = unsafe { CStr::from_ptr(addr.add(24)).to_string_lossy() };
    match TcpStream::connect(addr_str.as_ref()) {
        Ok(stream) => Box::into_raw(Box::new(PaceStream::Tcp(stream))) as *mut c_void,
        Err(_) => std::ptr::null_mut(),
    }
}

lazy_static::lazy_static! {
    static ref TLS_CONFIG: Arc<ClientConfig> = {
        let root_store = RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.into(),
        };
        Arc::new(
            ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth()
        )
    };
}

#[unsafe(no_mangle)]
pub extern "C" fn paceTlsConnect(addr: *const c_char, domain: *const c_char) -> *mut c_void {
    if addr.is_null() || domain.is_null() {
        return std::ptr::null_mut();
    }
    
    let addr_str = unsafe { CStr::from_ptr(addr.add(24)).to_string_lossy() };
    let domain_str = unsafe { CStr::from_ptr(domain.add(24)).to_string_lossy() };
    
    let server_name = match ServerName::try_from(domain_str.as_ref()) {
        Ok(s) => s.to_owned(),
        Err(_) => return std::ptr::null_mut(),
    };
    
    match TcpStream::connect(addr_str.as_ref()) {
        Ok(stream) => {
            match ClientConnection::new(TLS_CONFIG.clone(), server_name) {
                Ok(conn) => {
                    let tls_stream = StreamOwned::new(conn, stream);
                    Box::into_raw(Box::new(PaceStream::Tls(tls_stream))) as *mut c_void
                }
                Err(_) => std::ptr::null_mut(),
            }
        },
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn paceTcpBind(addr: *const c_char) -> *mut c_void {
    if addr.is_null() {
        return std::ptr::null_mut();
    }
    let addr_str = unsafe { CStr::from_ptr(addr.add(24)).to_string_lossy() };
    match TcpListener::bind(addr_str.as_ref()) {
        Ok(listener) => Box::into_raw(Box::new(listener)) as *mut c_void,
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn paceTcpAccept(listener: *mut c_void) -> *mut c_void {
    if listener.is_null() {
        return std::ptr::null_mut();
    }
    let listener = unsafe { &*(listener as *mut TcpListener) };
    match listener.accept() {
        Ok((stream, _)) => Box::into_raw(Box::new(PaceStream::Tcp(stream))) as *mut c_void,
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn paceTcpRead(stream: *mut c_void, buf: *mut u8, len: i64) -> i64 {
    if stream.is_null() || buf.is_null() || len <= 0 {
        return -1;
    }
    let stream = unsafe { &mut *(stream as *mut PaceStream) };
    let slice = unsafe { std::slice::from_raw_parts_mut(buf, len as usize) };
    match stream.read(slice) {
        Ok(n) => n as i64,
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn paceTcpWrite(stream: *mut c_void, buf: *const u8, len: i64) -> i64 {
    if stream.is_null() || buf.is_null() || len <= 0 {
        return -1;
    }
    let stream = unsafe { &mut *(stream as *mut PaceStream) };
    let slice = unsafe { std::slice::from_raw_parts(buf, len as usize) };
    match stream.write_all(slice) {
        Ok(_) => len,
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn paceTcpClose(stream: *mut c_void) {
    if stream.is_null() {
        return;
    }
    unsafe {
        let _ = Box::from_raw(stream as *mut PaceStream);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn paceTcpListenerClose(listener: *mut c_void) {
    if listener.is_null() {
        return;
    }
    unsafe {
        let _ = Box::from_raw(listener as *mut TcpListener);
    }
}

// Add string-specific read and write
#[unsafe(no_mangle)]
pub extern "C" fn paceTcpWriteString(stream: *mut c_void, string_ptr: *const c_char) -> i64 {
    if stream.is_null() || string_ptr.is_null() {
        return -1;
    }
    let stream = unsafe { &mut *(stream as *mut PaceStream) };
    let c_str = unsafe { CStr::from_ptr(string_ptr.add(24)) };
    let bytes = c_str.to_bytes();
    match stream.write_all(bytes) {
        Ok(_) => bytes.len() as i64,
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn paceTcpReadString(stream: *mut c_void, max_len: i64) -> *mut c_void {
    if stream.is_null() || max_len <= 0 {
        return std::ptr::null_mut();
    }
    let stream = unsafe { &mut *(stream as *mut PaceStream) };
    let mut buf = vec![0u8; max_len as usize];
    match stream.read(&mut buf) {
        Ok(n) if n > 0 => {
            buf.truncate(n);
            buf.push(0); // Null terminator
            let total_size = 24 + buf.len();
            unsafe {
                let out_ptr = crate::calloc(1, total_size);
                std::ptr::copy_nonoverlapping(buf.as_ptr(), out_ptr.add(24), buf.len());
                out_ptr as *mut c_void
            }
        },
        _ => std::ptr::null_mut(),
    }
}
