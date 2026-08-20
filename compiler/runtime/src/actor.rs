use std::ffi::c_void;
use tokio::sync::mpsc;

use crate::async_rt::PaceTask;

#[derive(Clone, Copy)]
pub struct ActorMailboxMessage(*mut c_void);

unsafe impl Send for ActorMailboxMessage {}
unsafe impl Sync for ActorMailboxMessage {}

pub struct PaceActorMailbox {
    sender: mpsc::UnboundedSender<ActorMailboxMessage>,
}

pub struct PaceActorState {
    receiver: mpsc::UnboundedReceiver<ActorMailboxMessage>,

}

unsafe impl Send for PaceActorMailbox {}
unsafe impl Sync for PaceActorMailbox {}
unsafe impl Send for PaceActorState {}

#[unsafe(no_mangle)]
pub extern "C" fn pace_actor_mailbox_create(
    _actor_instance_ptr: *mut c_void,
) -> *mut PaceActorMailbox {
    let (tx, rx) = mpsc::unbounded_channel();
    
    let mailbox = Box::new(PaceActorMailbox { sender: tx });
    let state = PaceActorState {
        receiver: rx,
    };
    
    // Spawn the background task to drain the mailbox
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async move {
            let mut state = state;
            while let Some(msg) = state.receiver.recv().await {
                let task = PaceTask { task_ptr: msg.0 };
                task.await;
            }
        });
    } else {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let mut state = state;
                while let Some(msg) = state.receiver.recv().await {
                    let task = PaceTask { task_ptr: msg.0 };
                    task.await;
                }
            });
        });
    }

    Box::into_raw(mailbox)
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_actor_mailbox_push(
    mailbox_ptr: *mut PaceActorMailbox,
    message: *mut c_void,
) {
    if mailbox_ptr.is_null() {
        return;
    }
    
    let mailbox = unsafe { &*mailbox_ptr };
    let _ = mailbox.sender.send(ActorMailboxMessage(message));
}

#[unsafe(no_mangle)]
pub extern "C" fn pace_actor_mailbox_destroy(mailbox_ptr: *mut PaceActorMailbox) {
    if !mailbox_ptr.is_null() {
        unsafe {
            let _ = Box::from_raw(mailbox_ptr);
        }
    }
}
