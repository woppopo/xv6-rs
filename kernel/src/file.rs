use arrayvec::ArrayVec;

use crate::{fs::NDIRECT, param::NFILE, pipe::Pipe, sleeplock::SleepLockC, spinlock::SpinLockC};

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

impl File {
    pub const fn new() -> Self {
        Self {
            kind: FileKind::None,
            ref_count: 0,
            readable: 0,
            writable: 0,
            pipe: core::ptr::null(),
            ip: core::ptr::null(),
            offset: 0,
        }
    }
}

// in-memory copy of an inode
#[repr(C)]
pub struct INode {
    dev: u32,         // Device number
    inum: u32,        // Inode number
    ref_count: u32,   //   Reference count
    lock: SleepLockC, // protects everything below here
    valid: i32,       // inode has been read from disk?

    ty: u16, // copy of disk inode
    major: u16,
    minor: u16,
    nlink: u16,
    size: u32,
    addrs: [u32; NDIRECT + 1],
}

struct FileTable {
    lock: SpinLockC,
    files: ArrayVec<File, NFILE>,
}

impl FileTable {
    pub const fn new() -> Self {
        Self {
            lock: SpinLockC::new(),
            files: ArrayVec::new_const(),
        }
    }

    // Allocate a file structure.
    pub fn alloc(&mut self) -> *mut File {
        self.lock.acquire();
        self.files.try_push(File::new()).unwrap();
        let file = self.files.last_mut().unwrap();
        file.ref_count += 1;
        self.lock.release();
        file
    }

    // Increment ref count for file f.
    pub fn dup(&mut self, f: &mut File) -> *mut File {
        self.lock.acquire();
        if f.ref_count < 1 {
            panic!("filedup");
        }
        f.ref_count += 1;
        self.lock.release();
        f
    }

    // Close file f.  (Decrement ref count, close when reaches 0.)
    pub fn close(&mut self, f: &mut File) {
        todo!()
    }
}
