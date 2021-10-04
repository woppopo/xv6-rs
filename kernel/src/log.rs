use crate::{fs::BSIZE, param::LOGSIZE, spinlock::SpinLock};

// Simple logging that allows concurrent FS system calls.
//
// A log transaction contains the updates of multiple FS system
// calls. The logging system only commits when there are
// no FS system calls active. Thus there is never
// any reasoning required about whether a commit might
// write an uncommitted system call's updates to disk.
//
// A system call should call begin_op()/end_op() to mark
// its start and end. Usually begin_op() just increments
// the count of in-progress FS system calls and returns.
// But if it thinks the log is close to running out, it
// sleeps until the last outstanding end_op() commits.
//
// The log is a physical re-do log containing disk blocks.
// The on-disk log format:
//   header block, containing block #s for block A, B, C, ...
//   block A
//   block B
//   block C
//   ...
// Log appends are synchronous.

// Contents of the header block, used for both the on-disk header block
// and to keep track in memory of logged block# before commit.
#[repr(C)]
pub struct LogHeader {
    n: i32,
    block: [i32; LOGSIZE],
}

#[repr(C)]
pub struct Log {
    lock: SpinLock,
    start: i32,
    size: i32,
    outstanding: i32, // how many FS sys calls are executing.
    committing: i32,  // in commit(), please wait.
    dev: i32,
    lh: LogHeader,
}
