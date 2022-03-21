use core::ffi::c_void;

use crate::{
    file::{File, FileKind},
    kalloc::{kalloc, kfree},
    proc::{my_process, sleep, wakeup},
    spinlock::SpinLockC,
};

const PIPESIZE: usize = 512;

#[repr(C)]
pub struct Pipe {
    lock: SpinLockC,
    data: [u8; PIPESIZE],
    nread: usize,   // number of bytes read
    nwrite: usize,  // number of bytes written
    readopen: i32,  // read fd is still open
    writeopen: i32, // write fd is still open
}

extern "C" {
    fn filealloc() -> *mut File;
    fn fileclose(f: *mut File);
}

#[no_mangle]
pub unsafe extern "C" fn pipealloc(f0: *mut *mut File, f1: *mut *mut File) -> i32 {
    *f0 = filealloc();
    *f1 = filealloc();

    if (*f0).is_null() || (*f1).is_null() {
        if !(*f0).is_null() {
            fileclose(*f0);
        }
        if !(*f1).is_null() {
            fileclose(*f1);
        }
        return -1;
    }

    let Some(p) = kalloc() else {
        fileclose(*f0);
        fileclose(*f1);
        return -1;
    };

    let p = p as *mut Pipe;
    (*p).readopen = 1;
    (*p).writeopen = 1;
    (*p).nread = 0;
    (*p).nwrite = 0;
    (*p).lock = SpinLockC::new();

    (*(*f0)).kind = FileKind::Pipe;
    (*(*f0)).readable = 1;
    (*(*f0)).writable = 0;
    (*(*f0)).pipe = p;

    (*(*f1)).kind = FileKind::Pipe;
    (*(*f1)).readable = 0;
    (*(*f1)).writable = 1;
    (*(*f1)).pipe = p;

    0
}

#[no_mangle]
pub unsafe extern "C" fn pipeclose(p: *mut Pipe, writable: i32) {
    (*p).lock.acquire();
    if writable != 0 {
        (*p).writeopen = 0;
        wakeup(&(*p).nread as *const _ as *const c_void);
    } else {
        (*p).readopen = 0;
        wakeup(&(*p).nwrite as *const _ as *const c_void);
    }

    if (*p).readopen == 0 && (*p).writeopen == 0 {
        (*p).lock.release();
        kfree(p as _);
    } else {
        (*p).lock.release();
    }
}

#[no_mangle]
pub unsafe extern "C" fn pipewrite(p: *mut Pipe, addr: *const u8, n: usize) -> i32 {
    (*p).lock.acquire();
    for i in 0..n {
        while (*p).nwrite == (*p).nread + PIPESIZE {
            if (*p).readopen == 0 || (*my_process().unwrap()).killed != 0 {
                (*p).lock.release();
                return -1;
            }
            wakeup(&(*p).nread as *const _ as *const c_void);
            sleep(&(*p).nwrite as *const _ as *const c_void, &(*p).lock);
        }
        (*p).data[(*p).nwrite as usize % PIPESIZE] = *addr.add(i);
        (*p).nwrite += 1;
    }
    wakeup(&(*p).nread as *const _ as *const c_void);
    (*p).lock.release();
    return n as i32;
}

#[no_mangle]
pub unsafe extern "C" fn piperead(p: *mut Pipe, addr: *mut u8, n: usize) -> i32 {
    (*p).lock.acquire();
    while (*p).nread == (*p).nwrite && (*p).writeopen != 0 {
        if (*my_process().unwrap()).killed != 0 {
            (*p).lock.release();
            return -1;
        }
        sleep(&(*p).nread as *const _ as *const c_void, &(*p).lock);
    }
    for i in 0..n {
        if (*p).nread == (*p).nwrite {
            wakeup(&(*p).nwrite as *const _ as *const c_void);
            (*p).lock.release();
            return i as i32;
        }
        *addr.add(i) = (*p).data[(*p).nread as usize % PIPESIZE];
        (*p).nread += 1;
    }
    wakeup(&(*p).nwrite as *const _ as *const c_void);
    (*p).lock.release();
    return n as i32;
}
