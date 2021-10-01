use crate::proc::Cpu;

// Mutual exclusion lock.
#[repr(C)]
pub struct SpinLock {
    locked: u32, // Is the lock held?

    // For debugging:
    name: *const char, // Name of lock.
    cpu: *const Cpu,   // The cpu holding the lock.
    pcs: [u32; 10],    // The call stack (an array of program counters)
                       // that locked the lock.
}