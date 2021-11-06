use crate::proc::myproc;

// User code makes a system call with INT T_SYSCALL.
// System call number in %eax.
// Arguments on the stack, from the user call to the C
// library system call function. The saved user %esp points
// to a saved program counter, and then the first argument.

#[repr(u8)]
pub enum SystemCall {
    Fork = 1,
    Exit = 2,
    Wait = 3,
    Pipe = 4,
    Read = 5,
    Kill = 6,
    Exec = 7,
    FileStatus = 8,
    ChangeDirectory = 9,
    Duplicate = 10,
    GetProcessID = 11,
    SpaceBreak = 12,
    Sleep = 13,
    UpTime = 14,
    Open = 15,
    Write = 16,
    MakeNode = 17,
    Unlink = 18,
    Link = 19,
    MakeDirectory = 20,
    Close = 21,
}

// Fetch the int at addr from the current process.
#[no_mangle]
extern "C" fn fetchint(addr: usize, ip: *mut i32) -> i32 {
    let mut curproc = unsafe { &mut *myproc() };
    if addr >= curproc.sz || addr + 4 > curproc.sz {
        return -1;
    }
    unsafe {
        *ip = *(addr as *mut i32);
    }
    0
}

fn strlen(p: *const char) -> usize {
    let mut i = 0;
    while unsafe { *p.add(i) } != '\0' {
        i += 1;
    }
    i
}

// Fetch the nul-terminated string at addr from the current process.
// Doesn't actually copy the string - just sets *pp to point at it.
// Returns length of string, not including nul.
#[no_mangle]
extern "C" fn fetchstr(addr: usize, pp: *mut *mut char) -> i32 {
    let mut curproc = unsafe { &mut *myproc() };
    if addr >= curproc.sz {
        return -1;
    }

    unsafe {
        *pp = addr as *mut char;
        strlen(*pp) as i32
    }
}

// Fetch the nth 32-bit system call argument.
#[no_mangle]
extern "C" fn argint(n: u32, ip: *mut i32) -> i32 {
    fetchint(unsafe { (*(*myproc()).tf).esp + 4 + 4 * n } as usize, ip)
}

// Fetch the nth word-sized system call argument as a pointer
// to a block of memory of size bytes.  Check that the pointer
// lies within the process address space.
#[no_mangle]
extern "C" fn argptr(n: u32, pp: *mut *mut u8, size: usize) -> i32 {
    let mut curproc = unsafe { &mut *myproc() };
    let mut i = 0;
    if argint(n, &mut i) < 0 {
        return -1;
    }
    if (i as usize) >= curproc.sz || (i as usize) + (size as usize) > curproc.sz {
        return -1;
    }
    unsafe {
        *pp = i as *mut u8;
    }
    0
}

// Fetch the nth word-sized system call argument as a string pointer.
// Check that the pointer is valid and the string is nul-terminated.
// (There is no shared writable memory, so the string can't change
// between this check and being used by the kernel.)
#[no_mangle]
extern "C" fn argstr(n: u32, pp: *mut *mut char) -> i32 {
    let mut curproc = unsafe { &mut *myproc() };
    let mut addr = 0;
    if argint(n, &mut addr) < 0 {
        return -1;
    }
    fetchstr(addr as usize, pp)
}

#[no_mangle]
pub extern "C" fn syscall() {
    extern "C" {
        fn sys_fork() -> u32;
        fn sys_exit() -> u32;
        fn sys_wait() -> u32;
        fn sys_pipe() -> u32;
        fn sys_read() -> u32;
        fn sys_kill() -> u32;
        fn sys_exec() -> u32;
        fn sys_fstat() -> u32;
        fn sys_chdir() -> u32;
        fn sys_dup() -> u32;
        fn sys_getpid() -> u32;
        fn sys_sbrk() -> u32;
        fn sys_sleep() -> u32;
        fn sys_uptime() -> u32;
        fn sys_open() -> u32;
        fn sys_write() -> u32;
        fn sys_mknod() -> u32;
        fn sys_unlink() -> u32;
        fn sys_link() -> u32;
        fn sys_mkdir() -> u32;
        fn sys_close() -> u32;
    }

    const SYSCALLS: [unsafe extern "C" fn() -> u32; 21] = [
        sys_fork, sys_exit, sys_wait, sys_pipe, sys_read, sys_kill, sys_exec, sys_fstat, sys_chdir,
        sys_dup, sys_getpid, sys_sbrk, sys_sleep, sys_uptime, sys_open, sys_write, sys_mknod,
        sys_unlink, sys_link, sys_mkdir, sys_close,
    ];

    let mut curproc = unsafe { &mut *myproc() };
    let mut tf = unsafe { &mut *curproc.tf };
    let num = tf.eax as usize;

    if let Some(syscall) = SYSCALLS.get(num - 1) {
        tf.eax = unsafe { syscall() };
    } else {
        panic!("{}: unknown syscall {}", curproc.pid, num);
    }
}
