use core::{
    cell::UnsafeCell,
    ffi::c_void,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

use crate::{
    interrupt::{free_from_interrupt, pop_cli, push_cli},
    memlayout::KERNBASE,
    proc::{my_cpu, Cpu},
};

pub struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<T> {
        push_cli();

        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {}

        SpinLockGuard::new(self)
    }

    fn release(&self) {
        self.locked.store(false, Ordering::SeqCst);
        pop_cli();
    }
}

pub struct SpinLockGuard<'l, T> {
    lock: &'l SpinLock<T>,
}

impl<'l, T> SpinLockGuard<'l, T> {
    pub fn new(lock: &'l SpinLock<T>) -> Self {
        Self { lock }
    }
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.release();
    }
}

// Mutual exclusion lock.
#[repr(C)]
pub struct SpinLockC {
    locked: AtomicU32, // Is the lock held?

    // For debugging:
    name: *const i8, // Name of lock.
    cpu: *const Cpu, // The cpu holding the lock.
    pcs: [u32; 10],  // The call stack (an array of program counters) that locked the lock.
}

impl SpinLockC {
    pub const fn new() -> Self {
        Self {
            locked: AtomicU32::new(0),
            name: core::ptr::null(),
            cpu: core::ptr::null(),
            pcs: [0; 10],
        }
    }

    // Acquire the lock.
    // Loops (spins) until the lock is acquired.
    // Holding a lock for a long time may cause
    // other CPUs to waste time spinning to acquire it.
    pub fn acquire(&mut self) {
        push_cli();

        assert!(!self.is_locked());

        while self
            .locked
            .compare_exchange_weak(
                0,
                1,
                core::sync::atomic::Ordering::SeqCst,
                core::sync::atomic::Ordering::Relaxed,
            )
            .is_err()
        {}

        // Record info about lock acquisition for debugging.
        self.cpu = my_cpu();
        //get_call_stack(&self as *const _ as *const u32, &mut self.pcs);
    }

    // Release the lock.
    pub fn release(&mut self) {
        assert!(self.is_locked());

        self.pcs[0] = 0;
        self.cpu = core::ptr::null();

        self.locked.store(0, core::sync::atomic::Ordering::SeqCst);

        pop_cli();
    }

    // Check whether this cpu is holding the lock.
    pub fn is_locked(&self) -> bool {
        free_from_interrupt(|| {
            self.locked.load(core::sync::atomic::Ordering::Relaxed) != 0 && self.cpu == my_cpu()
        })
    }
}

// Record the current call stack in pcs[] by following the %ebp chain.
fn get_call_stack(v: *const u32, pcs: &mut [u32]) {
    unsafe {
        let mut ebp = v.sub(2);
        for pc in pcs {
            if *ebp == 0 || (ebp as usize) < KERNBASE || (ebp as usize) == 0xffffffff {
                *pc = 0;
                continue;
            }

            *pc = *ebp.add(1); // saved %eip
            ebp = *ebp.add(0) as *const u32; // saved %ebp
        }
    }
}

mod _binding {
    use super::*;

    #[no_mangle]
    extern "C" fn initlock(lk: *mut SpinLockC, _name: *const i8) {
        unsafe {
            *lk = SpinLockC::new();
        }
    }

    #[no_mangle]
    extern "C" fn acquire(lock: *mut SpinLockC) {
        unsafe {
            (*lock).acquire();
        }
    }

    #[no_mangle]
    extern "C" fn release(lock: *mut SpinLockC) {
        unsafe {
            (*lock).release();
        }
    }

    #[no_mangle]
    extern "C" fn holding(lock: *const SpinLockC) -> i32 {
        unsafe {
            match (*lock).is_locked() {
                true => 1,
                false => 0,
            }
        }
    }

    #[no_mangle]
    extern "C" fn getcallerpcs(v: *const c_void, pcs: *mut u32) {
        unsafe {
            let v = v as *const u32;
            let pcs = core::slice::from_raw_parts_mut(pcs, 10);
            get_call_stack(v, pcs);
        }
    }
}
