use core::ffi::c_void;

use crate::mmu::PGSIZE;

pub fn kalloc() -> Option<usize> {
    extern "C" {
        fn kalloc() -> *mut c_void;
    }

    let addr = unsafe { kalloc() };
    if !addr.is_null() {
        Some(addr as usize)
    } else {
        None
    }
}

pub fn kalloc_zeroed() -> Option<usize> {
    let addr = kalloc()?;
    unsafe {
        core::ptr::write_bytes(addr as *mut u8, 0, PGSIZE);
    }
    Some(addr)
}

pub fn kfree(v: usize) {
    extern "C" {
        fn kfree(v: *mut i8);
    }

    unsafe {
        kfree(v as *mut i8);
    }
}
