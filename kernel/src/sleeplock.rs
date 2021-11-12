use core::sync::atomic::AtomicBool;

use crate::{
    proc::{myproc, sleep, wakeup},
    spinlock::SpinLockC,
};

// Long-term locks for processes
#[repr(C)]
pub struct SleepLockC {
    locked: u32,     // Is the lock held?
    lock: SpinLockC, // spinlock protecting this sleep lock
    pid: i32,        // Process holding lock
}

impl SleepLockC {
    pub const fn new() -> Self {
        Self {
            locked: 0,
            lock: SpinLockC::new(),
            pid: 0,
        }
    }

    pub fn acquire(&mut self) {
        self.lock.acquire();
        while self.locked != 0 {
            unsafe {
                sleep(self as *const Self as *const _, &self.lock);
            }
        }
        self.locked = 1;
        self.pid = unsafe { (*myproc()).pid };
        self.lock.release();
    }

    pub fn release(&mut self) {
        self.lock.acquire();
        self.locked = 0;
        self.pid = 0;
        unsafe {
            wakeup(self as *const Self as *const _);
        }
        self.lock.release();
    }

    pub fn is_locked(&mut self) -> bool {
        self.lock.acquire();
        let ret = self.locked != 0 && self.pid == unsafe { (*myproc()).pid };
        self.lock.release();
        ret
    }
}

mod _binding {
    use super::*;

    #[no_mangle]
    extern "C" fn initsleeplock(lk: *mut SleepLockC, _name: *const i8) {
        unsafe {
            *lk = SleepLockC::new();
        }
    }

    #[no_mangle]
    extern "C" fn acquiresleep(lk: *mut SleepLockC) {
        unsafe {
            (*lk).acquire();
        }
    }

    #[no_mangle]
    extern "C" fn releasesleep(lk: *mut SleepLockC) {
        unsafe {
            (*lk).release();
        }
    }

    #[no_mangle]
    extern "C" fn holdingsleep(lk: *mut SleepLockC) -> i32 {
        unsafe {
            match (*lk).is_locked() {
                true => 1,
                false => 0,
            }
        }
    }
}
