use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::ffi::c_void;

type PacePollFn = extern "C" fn(*mut c_void, *mut Waker) -> i32;

pub struct PaceTask {
    pub task_ptr: *mut c_void,
}

unsafe impl Send for PaceTask {}
unsafe impl Sync for PaceTask {}

impl Future for PaceTask {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker = cx.waker().clone();
        let waker_ptr = Box::into_raw(Box::new(waker));
        
        // Read context_ptr and poll_fn from Task object
        // Task fields: ["state" (offset 24), "context" (offset 32), "poll_fn" (offset 40), "waker" (offset 48), "result" (offset 56)]
        let context_ptr = unsafe {
            let ctx_ptr_loc = (self.task_ptr as *const u8).add(32) as *const *mut c_void;
            *ctx_ptr_loc
        };
        
        let poll_fn = unsafe {
            let poll_fn_loc = (self.task_ptr as *const u8).add(40) as *const PacePollFn;
            *poll_fn_loc
        };
        
        let result = (poll_fn)(context_ptr, waker_ptr);
        
        if result == 1 {
            // Read waker from Task object and wake it if it exists
            let stored_waker_ptr = unsafe {
                let waker_loc = (self.task_ptr as *const u8).add(48) as *const *mut Waker;
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
pub extern "C" fn paceSpawnTask(task_ptr: *mut c_void) {
    if task_ptr.is_null() { return; }
    
    // Retain the task object so it doesn't get freed while we execute it
    crate::pace_retain(task_ptr as *mut u8);
    
    let task = PaceTask {
        task_ptr,
    };
    
    let task_ptr_val = task_ptr as usize;
    let future = async move {
        task.await;
        // Release the task object after it's done executing
        crate::pace_release(task_ptr_val as *mut u8);
    };
    
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    let rt = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
    });
    
    rt.spawn(future);
}

#[unsafe(no_mangle)]
pub extern "C" fn paceWakerWake(waker_ptr: *mut c_void) {
    if waker_ptr.is_null() {
        return;
    }
    unsafe {
        let waker = Box::from_raw(waker_ptr as *mut Waker);
        waker.wake();
    }
}
