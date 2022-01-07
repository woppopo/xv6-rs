use core::ffi::c_void;

use crate::x86::{stosb, stosl};

#[no_mangle]
unsafe extern "C" fn memset(dst: *mut c_void, mut c: u32, n: usize) -> *mut c_void {
    if dst.align_offset(4) == 0 && n % 4 == 0 {
        c &= 0xFF;
        stosl(dst.cast(), (c << 24) | (c << 16) | (c << 8) | c, n / 4);
    } else {
        stosb(dst.cast(), c as u8, n);
    }
    dst
}

#[no_mangle]
unsafe extern "C" fn memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 {
    for i in (0..n).rev() {
        let a = *(s1.add(i) as *const u8);
        let b = *(s2.add(i) as *const u8);
        if a != b {
            return a as i32 - b as i32;
        }
    }
    0
}

#[no_mangle]
unsafe extern "C" fn memmove(dst: *mut c_void, src: *const c_void, mut n: usize) -> *mut c_void {
    let mut s = src as usize;
    let mut d = dst as usize;
    if s < d && s + n > d {
        s += n;
        d += n;
        for _i in 0..n {
            d -= 1;
            s -= 1;
            *(d as *mut u8) = *(s as *mut u8);
            n -= 1;
        }
    } else {
        for _i in 0..n {
            *(d as *mut u8) = *(s as *mut u8);
            d += 1;
            s += 1;
            n -= 1;
        }
    }
    dst
}

#[no_mangle]
unsafe extern "C" fn strncmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a == 0 || a != b {
            return a as i32 - b as i32;
        }
    }
    0
}

#[no_mangle]
unsafe extern "C" fn strncpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let src_len = strlen(src);
    for i in 0..n {
        if i < src_len {
            *dst.add(i) = *src.add(i);
        } else {
            *dst.add(i) = 0;
        }
    }
    dst
}

#[no_mangle]
unsafe extern "C" fn safestrcpy(s: *mut u8, t: *const u8, n: usize) -> *mut u8 {
    s.write_bytes(0, n);
    s.copy_from_nonoverlapping(t, strlen(t));
    *s.add(n - 1) = 0;
    s
}

#[no_mangle]
unsafe extern "C" fn strlen(s: *const u8) -> usize {
    let mut i = 0;
    while *s.add(i) != 0 {
        i += 1;
    }
    i
}
