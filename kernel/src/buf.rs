use arrayvec::ArrayVec;

use crate::{fs::BSIZE, ide::IDE, param::NBUF, sleeplock::SleepLock, spinlock::SpinLockC};

#[repr(C)]
pub struct Buffer {
    pub flags: i32,
    pub dev: usize,
    pub blockno: usize,
    pub lock: SleepLock,
    refcnt: u32,
    prev: *mut Self,
    next: *mut Self,
    pub qnext: *mut Self,
    pub data: [u8; BSIZE],
}

impl Buffer {
    pub const VALID: i32 = 0x2; // buffer has been read from disk
    pub const DIRTY: i32 = 0x4; // buffer needs to be written to disk

    pub const fn new(dev: usize, blockno: usize) -> Self {
        Self {
            flags: 0,
            dev,
            blockno,
            lock: SleepLock::new(),
            refcnt: 1,
            prev: core::ptr::null_mut(),
            next: core::ptr::null_mut(),
            qnext: core::ptr::null_mut(),
            data: [0; BSIZE],
        }
    }

    // Write b's contents to disk.  Must be locked.
    pub fn write(&mut self) {
        if !self.lock.is_locked() {
            panic!("Buffer::write");
        }

        self.flags |= Buffer::DIRTY;

        unsafe {
            IDE.as_mut().unwrap().rw(self);
        }
    }

    pub fn release(&mut self, cache: &mut BufferCache) {
        if !self.lock.is_locked() {
            panic!("Buffer::release");
        }

        self.lock.release();

        cache.release_buffer(self);
    }
}

// Buffer cache.
//
// The buffer cache is a linked list of buf structures holding
// cached copies of disk block contents.  Caching disk blocks
// in memory reduces the number of disk reads and also provides
// a synchronization point for disk blocks used by multiple processes.
//
// Interface:
// * To get a buffer for a particular disk block, call bread.
// * After changing buffer data, call bwrite to write it to disk.
// * When done with the buffer, call brelse.
// * Do not use the buffer after calling brelse.
// * Only one process at a time can use a buffer,
//     so do not keep them longer than necessary.
//
// The implementation uses two state flags internally:
// * B_VALID: the buffer data has been read from the disk.
// * B_DIRTY: the buffer data has been modified
//     and needs to be written to disk.

pub struct BufferCache {
    lock: SpinLockC,
    buffers: ArrayVec<Buffer, NBUF>,
}

impl BufferCache {
    pub const fn new() -> Self {
        Self {
            lock: SpinLockC::new(),
            buffers: ArrayVec::new_const(),
        }
    }

    pub fn push(&mut self, dev: usize, blockno: usize) -> usize {
        let mut buf = Buffer::new(dev, blockno);
        buf.lock.acquire();

        if self.buffers.try_push(buf).is_err() {
            panic!("BufferCache::get: no buffer");
        }

        self.buffers.len() - 1
    }

    // Look through buffer cache for block on device dev.
    // If not found, allocate a buffer.
    // In either case, return locked buffer.
    pub fn get(&mut self, dev: usize, blockno: usize) -> &mut Buffer {
        self.lock.acquire();

        self.buffers
            .retain(|buf| buf.refcnt != 0 || buf.flags & Buffer::DIRTY != 0);

        let at = self
            .buffers
            .iter_mut()
            .position(|buf| buf.dev == dev && buf.blockno == blockno);

        // Is the block already cached?
        if let Some(at) = at {
            let buf = &mut self.buffers[at];
            buf.refcnt += 1;
            self.lock.release();
            buf.lock.acquire();
            buf
        } else {
            // Not cached; recycle an unused buffer.
            // Even if refcnt==0, B_DIRTY indicates a buffer is in use
            // because log.c has modified it but not yet committed it.
            let at = self.push(dev, blockno);
            self.lock.release();
            &mut self.buffers[at]
        }
    }

    // Return a locked buf with the contents of the indicated block.
    pub fn read(&mut self, dev: usize, blockno: usize) -> &mut Buffer {
        let buf = self.get(dev, blockno);
        if buf.flags & Buffer::VALID == 0 {
            unsafe {
                IDE.as_mut().unwrap().rw(buf);
            }
        }
        buf
    }

    pub fn release_buffer(&mut self, buf: &mut Buffer) {
        self.lock.acquire();
        buf.refcnt -= 1;
        if buf.refcnt == 0 && buf.flags & Buffer::DIRTY == 0 {
            let index = self
                .buffers
                .iter()
                .position(|v| v.dev == buf.dev && v.blockno == buf.blockno)
                .unwrap();

            self.buffers.swap_remove(index);
        }
        self.lock.release();
    }
}

mod _binding {
    use super::*;

    static mut CACHE: BufferCache = BufferCache::new();

    #[no_mangle]
    extern "C" fn bread(dev: u32, blockno: u32) -> *mut Buffer {
        unsafe { CACHE.read(dev as usize, blockno as usize) }
    }

    #[no_mangle]
    extern "C" fn bwrite(b: *mut Buffer) {
        unsafe {
            (*b).write();
        }
    }

    #[no_mangle]
    extern "C" fn brelse(b: *mut Buffer) {
        unsafe {
            (*b).release(&mut CACHE);
        }
    }
}
