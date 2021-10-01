use crate::spinlock::SpinLock;

// Long-term locks for processes
#[repr(C)]
pub struct SleepLock {
    locked: u32,  // Is the lock held?
    lk: SpinLock, // spinlock protecting this sleep lock

    // For debugging:
    name: *const char, // Name of lock.
    pid: i32,          // Process holding lock
}
