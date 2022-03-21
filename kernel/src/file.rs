use arrayvec::ArrayVec;

use crate::{
    fs::NDIRECT,
    param::{MAXOPBLOCKS, NFILE},
    pipe::{pipeclose, piperead, pipewrite, Pipe},
    sleeplock::SleepLockC,
    spinlock::SpinLockC,
    stat::Stat,
};

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub enum FileKind {
    None,
    Pipe,
    INode,
}

#[repr(C)]
#[derive(PartialEq)]
pub struct File {
    pub kind: FileKind,
    pub ref_count: i32, // reference count
    pub readable: u8,
    pub writable: u8,
    pub pipe: *mut Pipe,
    pub ip: *mut INode,
    pub offset: u32,
}

impl File {
    pub const fn new() -> Self {
        Self {
            kind: FileKind::None,
            ref_count: 0,
            readable: 0,
            writable: 0,
            pipe: core::ptr::null_mut(),
            ip: core::ptr::null_mut(),
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
        self.lock.acquire();
        if f.ref_count < 1 {
            panic!("fileclose");
        }
        f.ref_count -= 1;
        if f.ref_count == 0 {
            let ty = f.kind;
            let pipe = f.pipe;
            let writable = f.writable;
            let ip = f.ip;

            let index = self.files.iter().position(|file| file == f).unwrap();
            self.files.remove(index);

            match ty {
                FileKind::Pipe => unsafe {
                    pipeclose(pipe, writable as i32);
                },
                FileKind::INode => unsafe {
                    begin_op();
                    iput(ip);
                    end_op();
                },
                _ => {}
            }
        }
        self.lock.release();
    }

    pub fn stat(&self, f: &File, st: &mut Stat) -> i32 {
        if f.kind != FileKind::INode {
            return -1;
        }

        unsafe {
            ilock(f.ip);
            stati(f.ip, st);
            iunlock(f.ip);
        }
        0
    }

    pub fn read(&self, f: &mut File, buf: &mut [u8]) -> i32 {
        if f.readable == 0 {
            return -1;
        }

        match f.kind {
            FileKind::Pipe => unsafe {
                unsafe {
                    return piperead(f.pipe, buf.as_mut_ptr(), buf.len());
                }
            },
            FileKind::INode => unsafe {
                ilock(f.ip);
                let read = readi(f.ip, buf.as_mut_ptr(), f.offset, buf.len() as u32);
                if read > 0 {
                    f.offset += read;
                }
                iunlock(f.ip);
                return read as i32;
            },
            _ => panic!("fileread"),
        }
    }

    pub fn write(&self, f: &mut File, buf: &[u8]) -> i32 {
        if f.writable == 0 {
            return -1;
        }

        match f.kind {
            FileKind::Pipe => unsafe {
                unsafe {
                    return pipewrite(f.pipe, buf.as_ptr(), buf.len());
                }
            },
            FileKind::INode => unsafe {
                // write a few blocks at a time to avoid exceeding
                // the maximum log transaction size, including
                // i-node, indirect block, allocation blocks,
                // and 2 blocks of slop for non-aligned writes.
                // this really belongs lower down, since writei()
                // might be writing a device like the console.
                const MAX: usize = ((MAXOPBLOCKS - 1 - 1 - 2) / 2) * 512;
                let mut i = 0;
                let n = buf.len();
                while i < n {
                    let mut n1 = n - i;
                    if n1 > MAX {
                        n1 = MAX;
                    }

                    begin_op();
                    ilock(f.ip);
                    let read = writei(f.ip, buf.as_ptr().add(i), f.offset, n1 as u32);
                    if read > 0 {
                        f.offset += read as u32;
                    }
                    iunlock(f.ip);
                    end_op();

                    if read < 0 {
                        break;
                    }
                    if read as usize != n1 {
                        panic!("short filewrite");
                    }
                    i += read as usize;
                }
                return if i == n { n as i32 } else { -1 };
            },
            _ => panic!("filewrite"),
        }
    }
}

extern "C" {
    fn begin_op();
    fn iput(ip: *mut INode);
    fn ilock(ip: *mut INode);
    fn iunlock(ip: *mut INode);
    fn end_op();
    fn writei(ip: *mut INode, buf: *const u8, offset: u32, n: u32) -> i32;
    fn readi(ip: *mut INode, buf: *mut u8, offset: u32, n: u32) -> u32;
    fn stati(ip: *mut INode, st: *mut Stat);
}

static mut FILE_TABLE: FileTable = FileTable::new();

mod binding {
    use super::*;

    #[no_mangle]
    extern "C" fn fileinit() {}

    #[no_mangle]
    extern "C" fn filealloc() -> *mut File {
        unsafe { FILE_TABLE.alloc() }
    }

    #[no_mangle]
    extern "C" fn filedup(f: *mut File) -> *mut File {
        unsafe { FILE_TABLE.dup(&mut *f) }
    }

    #[no_mangle]
    extern "C" fn fileclose(f: *mut File) {
        unsafe { FILE_TABLE.close(&mut *f) }
    }

    #[no_mangle]
    extern "C" fn filestat(f: *mut File, st: *mut Stat) -> i32 {
        unsafe { FILE_TABLE.stat(&mut *f, &mut *st) }
    }

    #[no_mangle]
    extern "C" fn fileread(f: *mut File, buf: *mut u8, n: usize) -> i32 {
        unsafe { FILE_TABLE.read(&mut *f, core::slice::from_raw_parts_mut(buf, n)) }
    }

    #[no_mangle]
    extern "C" fn filewrite(f: *mut File, buf: *const u8, n: usize) -> i32 {
        unsafe { FILE_TABLE.write(&mut *f, core::slice::from_raw_parts(buf, n)) }
    }
}
