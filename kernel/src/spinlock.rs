use core::{ffi::c_void, sync::atomic::AtomicU32};

use crate::{
    memlayout::KERNBASE,
    mmu::FL_IF,
    proc::{mycpu, Cpu},
    x86::{cli, readeflags, sti},
};

// Mutual exclusion lock.
#[repr(C)]
pub struct SpinLock {
    locked: AtomicU32, // Is the lock held?

    // For debugging:
    name: *const i8, // Name of lock.
    cpu: *const Cpu, // The cpu holding the lock.
    pcs: [u32; 10],  // The call stack (an array of program counters) that locked the lock.
}

impl SpinLock {
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
        self.cpu = unsafe { mycpu() };
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
            self.locked.load(core::sync::atomic::Ordering::Relaxed) != 0
                && self.cpu == unsafe { mycpu() }
        })
    }
}

pub fn free_from_interrupt<R>(f: impl FnOnce() -> R) -> R {
    push_cli();
    let ret = f();
    pop_cli();
    ret
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

// Pushcli/popcli are like cli/sti except that they are matched:
// it takes two popcli to undo two pushcli.  Also, if interrupts
// are off, then pushcli, popcli leaves them off.
fn push_cli() {
    let eflags = unsafe { readeflags() };
    unsafe {
        cli();
    }

    let cpu = unsafe { &mut *mycpu() };
    if cpu.ncli == 0 {
        cpu.intena = eflags & FL_IF;
    }
    cpu.ncli += 1;
}

fn pop_cli() {
    if unsafe { readeflags() & FL_IF != 0 } {
        panic!("pop_cli - interruptible");
    }

    let cpu = unsafe { &mut *mycpu() };
    if cpu.ncli == 0 {
        panic!("pop_cli");
    }

    cpu.ncli -= 1;

    if cpu.ncli == 0 && cpu.intena != 0 {
        unsafe {
            sti();
        }
    }
}

mod _binding {
    use super::*;

    #[no_mangle]
    extern "C" fn initlock(lk: *mut SpinLock, _name: *const i8) {
        unsafe {
            *lk = SpinLock::new();
        }
    }

    #[no_mangle]
    extern "C" fn acquire(lock: *mut SpinLock) {
        unsafe {
            (*lock).acquire();
        }
    }

    #[no_mangle]
    extern "C" fn release(lock: *mut SpinLock) {
        unsafe {
            (*lock).release();
        }
    }

    #[no_mangle]
    extern "C" fn holding(lock: *const SpinLock) -> i32 {
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

    #[no_mangle]
    extern "C" fn pushcli() {
        push_cli();
    }

    #[no_mangle]
    extern "C" fn popcli() {
        pop_cli();
    }
}
