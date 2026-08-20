use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::ffi::c_void;

type PacePollFn = extern "C" fn(*mut c_void, *mut Waker) -> i32;

pub struct PaceTask {
    task_ptr: *mut c_void,
}

unsafe impl Send for PaceTask {}
unsafe impl Sync for PaceTask {}

impl Future for PaceTask {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker = cx.waker().clone();
        let waker_ptr = Box::into_raw(Box::new(waker));
        
        // Read context_ptr and poll_fn from Task object
        // Task fields: ["context" (offset 24), "poll_fn" (offset 32), "waker" (offset 40), "result" (offset 48)]
        let context_ptr = unsafe {
            let ctx_ptr_loc = (self.task_ptr as *const u8).add(24) as *const *mut c_void;
            *ctx_ptr_loc
        };
        
        let poll_fn = unsafe {
            let poll_fn_loc = (self.task_ptr as *const u8).add(32) as *const PacePollFn;
            *poll_fn_loc
        };
        
        let result = (poll_fn)(context_ptr, waker_ptr);
        
        if result == 1 {
            // Read waker from Task object and wake it if it exists
            let stored_waker_ptr = unsafe {
                let waker_loc = (self.task_ptr as *const u8).add(40) as *const *mut Waker;
                *waker_loc
            };
            
            if !stored_waker_ptr.is_null() {
                unsafe {
                    let stored_waker = Box::from_raw(stored_waker_ptr);
                    stored_waker.wake();
                }
            }
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_spawn_task(task_ptr: *mut c_void) {
    let task = PaceTask {
        task_ptr,
    };
    
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(task);
    } else {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(task);
        });
    }
}
