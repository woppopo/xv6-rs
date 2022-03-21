// On-disk file system format.
// Both the kernel and user programs use this header file.

use crate::file::INode;

pub const ROOTINO: usize = 1; // root i-number
pub const BSIZE: usize = 512; // block size

// Disk layout:
// [ boot block | super block | log | inode blocks |
//                                          free bit map | data blocks]
//
// mkfs computes the super block and builds an initial file system. The
// super block describes the disk layout:
pub struct SuperBlock {
    size: u32,       // Size of file system image (blocks)
    nblocks: u32,    // Number of data blocks
    ninodes: u32,    // Number of inodes.
    nlog: u32,       // Number of log blocks
    logstart: u32,   // Block number of first log block
    inodestart: u32, // Block number of first inode block
    bmapstart: u32,  // Block number of first free map block
}

pub const NDIRECT: usize = 12;

pub fn read_inode(ip: *const INode, dst: usize, offset: usize, n: usize) -> usize {
    extern "C" {
        fn readi(ip: *const INode, dst: *mut u8, off: u32, n: u32) -> i32;
    }

    unsafe { readi(ip, dst as *mut u8, offset as u32, n as u32) as usize }
}
