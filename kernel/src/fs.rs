use crate::file::INode;

pub const NDIRECT: usize = 12;

pub fn read_inode(ip: *const INode, dst: usize, offset: usize, n: usize) -> usize {
    extern "C" {
        fn readi(ip: *const u32, dst: *mut u8, off: u32, n: u32) -> i32;
    }

    unsafe { readi(ip as *const u32, dst as *mut u8, offset as u32, n as u32) as usize }
}
