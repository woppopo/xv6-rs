// On-disk file system format.
// Both the kernel and user programs use this header file.

use arrayvec::ArrayVec;

use crate::{
    buf::{Buffer, BUFFER_CACHE},
    file::INode,
    param::NINODE,
    spinlock::SpinLockC,
};

pub const ROOTINO: usize = 1; // root i-number
pub const BSIZE: usize = 512; // block size

// Disk layout:
// [ boot block | super block | log | inode blocks |
//                                          free bit map | data blocks]
//
// mkfs computes the super block and builds an initial file system. The
// super block describes the disk layout:
#[repr(C)]
#[derive(Clone)]
pub struct SuperBlock {
    size: usize,       // Size of file system image (blocks)
    nblocks: usize,    // Number of data blocks
    ninodes: usize,    // Number of inodes.
    nlog: usize,       // Number of log blocks
    logstart: usize,   // Block number of first log block
    inodestart: usize, // Block number of first inode block
    bmapstart: usize,  // Block number of first free map block
}

pub struct INodeCache {
    lock: SpinLockC,
    inodes: ArrayVec<INode, NINODE>,
}

pub const NDIRECT: usize = 12;
pub const NINDIRECT: usize = BSIZE / core::mem::size_of::<u32>();
pub const MAXFILE: usize = NDIRECT + NINDIRECT;

pub struct INodeOnDisk {
    kind: u16,
    major: u16,
    minor: u16,
    nlink: u16,
    size: u32,
    addrs: [u32; NDIRECT + 1],
}

const IPB: usize = BSIZE / core::mem::size_of::<INodeOnDisk>();

const fn iblock(i: usize, sb: &SuperBlock) -> usize {
    i / IPB + sb.inodestart
}

const BPB: usize = BSIZE * 8;

const fn bblock(b: usize, sb: &SuperBlock) -> usize {
    b / BPB + sb.bmapstart
}

const DIRSIZ: usize = 14;

struct DirectoryEntry {
    inum: u16,
    name: [u8; DIRSIZ],
}

extern "C" {
    fn log_write(buf: *mut Buffer);
}

pub fn read_inode(ip: *const INode, dst: usize, offset: usize, n: usize) -> usize {
    extern "C" {
        fn readi(ip: *const INode, dst: *mut u8, off: u32, n: u32) -> i32;
    }

    unsafe { readi(ip, dst as *mut u8, offset as u32, n as u32) as usize }
}

pub fn read_superblock(dev: usize) -> SuperBlock {
    unsafe {
        let buf = BUFFER_CACHE.read(dev, 1);
        let sb = buf.data.as_ptr() as *const SuperBlock;
        let sb = (*sb).clone();
        buf.release(&mut BUFFER_CACHE);
        sb
    }
}

pub fn zero_block(dev: usize, bno: usize) {
    unsafe {
        let buf = BUFFER_CACHE.read(dev, bno);
        core::ptr::write_bytes(buf.data.as_mut_ptr(), 0, buf.data.len());
        log_write(buf);
        buf.release(&mut BUFFER_CACHE);
    }
}

pub unsafe fn allocate_block(sb: &SuperBlock, dev: usize) -> usize {
    let mut b = 0;
    for b in (0..sb.size).step_by(BPB) {
        let buf = BUFFER_CACHE.read(dev, bblock(b, sb));
        let mut bi = 0;
        while bi < BPB && b + bi < sb.size {
            let m = 1 << (bi % 8);
            if (buf.data[bi / 8] & m) == 0 {
                buf.data[bi / 8] |= m;
                log_write(buf);
                buf.release(&mut BUFFER_CACHE);
                zero_block(dev, b + bi);
                return b + bi;
            }
            bi += 1;
        }
        buf.release(&mut BUFFER_CACHE);
    }
    panic!("out of blocks")
}

pub unsafe fn free_block(sb: &SuperBlock, dev: usize, bno: usize) {
    let buf = BUFFER_CACHE.read(dev, bblock(bno, sb));
    let bi = bno % BPB;
    let m = 1 << (bi % 8);
    if buf.data[bi / 8] & m == 0 {
        panic!("freeing free block");
    }
    buf.data[bi / 8] &= !m;
    log_write(buf);
    buf.release(&mut BUFFER_CACHE);
}

mod binding {
    use super::*;

    //#[no_mangle]
    extern "C" fn readsb(dev: usize, sb: *mut SuperBlock) {
        unsafe { *sb = read_superblock(dev) };
    }

    //#[no_mangle]
    extern "C" fn balloc(dev: usize) -> usize {
        let sb = read_superblock(dev);
        unsafe { allocate_block(&sb, dev) }
    }

    //#[no_mangle]
    extern "C" fn bfree(dev: usize, bno: usize) {
        let sb = read_superblock(dev);
        unsafe { free_block(&sb, dev, bno) };
    }
}
