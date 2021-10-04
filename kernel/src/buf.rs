use core::mem::MaybeUninit;

use heapless::Vec;

use crate::{fs::BSIZE, ide::iderw, param::NBUF, sleeplock::SleepLock, spinlock::SpinLock};

#[repr(C)]
pub struct Buffer {
    flags: i32,
    dev: usize,
    blockno: usize,
    lock: SleepLock,
    refcnt: u32,
    qnext: *mut Self,
    data: [u8; BSIZE],
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
            iderw(self);
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

pub struct BufferCache {
    lock: SpinLock,
    buffers: Vec<Buffer, NBUF>,
}

impl BufferCache {
    pub const fn new() -> Self {
        Self {
            lock: SpinLock::new(),
            buffers: Vec::new(),
        }
    }

    pub fn push(&mut self, dev: usize, blockno: usize) -> usize {
        let mut buf = Buffer::new(dev, blockno);
        buf.lock.acquire();

        if self.buffers.push(buf).is_err() {
            panic!("BufferCache::get: no buffer");
        }

        self.buffers.len() - 1
    }

    // Look through buffer cache for block on device dev.
    // If not found, allocate a buffer.
    // In either case, return locked buffer.
    pub fn get(&mut self, dev: usize, blockno: usize) -> &mut Buffer {
        self.lock.acquire();

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
                iderw(buf);
            }
        }
        buf
    }

    pub fn release_buffer(&mut self, buf: &mut Buffer) {
        self.lock.acquire();
        buf.refcnt -= 1;
        if buf.refcnt == 0 {
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

    /*
    #[no_mangle]
    extern "C" fn binit() {}

    #[no_mangle]
    extern "C" fn bget(dev: u32, blockno: u32) -> *mut Buffer {
        unsafe { CACHE.get(dev as usize, blockno as usize) }
    }

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
     */
}
