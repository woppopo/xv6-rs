use crate::{fs::NDIRECT, pipe::Pipe, sleeplock::SleepLock};

#[repr(C)]
pub enum FileKind {
    None,
    Pipe,
    INode,
}

#[repr(C)]
pub struct File {
    kind: FileKind,
    ref_count: i32, // reference count
    readable: u8,
    writable: u8,
    pipe: *const Pipe,
    ip: *const INode,
    offset: u32,
}

// in-memory copy of an inode
#[repr(C)]
pub struct INode {
    dev: u32,        // Device number
    inum: u32,       // Inode number
    ref_count: u32,  //   Reference count
    lock: SleepLock, // protects everything below here
    valid: i32,      // inode has been read from disk?

    ty: u16, // copy of disk inode
    major: u16,
    minor: u16,
    nlink: u16,
    size: u32,
    addrs: [u32; NDIRECT + 1],
}
