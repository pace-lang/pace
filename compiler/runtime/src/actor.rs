use std::ffi::c_void;
use tokio::sync::mpsc;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct ActorMailboxMessage(*mut c_void);

unsafe impl Send for ActorMailboxMessage {}
unsafe impl Sync for ActorMailboxMessage {}

pub struct PaceActorMailbox {
    sender: mpsc::UnboundedSender<ActorMailboxMessage>,
}

pub struct PaceActorState {
    receiver: mpsc::UnboundedReceiver<ActorMailboxMessage>,
    actor_instance_ptr: *mut c_void,
    process_message_fn: extern "C" fn(*mut c_void, *mut c_void),
}

unsafe impl Send for PaceActorMailbox {}
unsafe impl Sync for PaceActorMailbox {}
unsafe impl Send for PaceActorState {}

#[unsafe(no_mangle)]
pub extern "C" fn pace_actor_mailbox_create(
    actor_instance_ptr: *mut c_void,
    process_message_fn: extern "C" fn(*mut c_void, *mut c_void),
) -> *mut PaceActorMailbox {
    let (tx, rx) = mpsc::unbounded_channel();
    
    let mailbox = Box::new(PaceActorMailbox { sender: tx });
    let state = PaceActorState {
        receiver: rx,
        actor_instance_ptr,
        process_message_fn,
    };
    
    // Spawn the background task to drain the mailbox
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async move {
            let mut state = state;
            while let Some(msg) = state.receiver.recv().await {
                (state.process_message_fn)(state.actor_instance_ptr, msg.0);
            }
        });
    } else {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let mut state = state;
                while let Some(msg) = state.receiver.recv().await {
                    (state.process_message_fn)(state.actor_instance_ptr, msg.0);
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
