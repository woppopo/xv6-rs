use crate::{fs::BSIZE, sleeplock::SleepLock};

pub struct Buffer {
    flags: i32,
    dev: u32,
    blockno: u32,
    lock: SleepLock,
    refcnt: u32,
    prev: *mut Self, // LRU cache list
    next: *mut Self,
    qnext: *mut Self, // disk queue
    data: [u8; BSIZE],
}

impl Buffer {
    pub const VALID: u8 = 0x2; // buffer has been read from disk
    pub const DIRTY: u8 = 0x4; // buffer needs to be written to disk
}
