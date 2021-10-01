use crate::spinlock::SpinLock;

const PIPESIZE: usize = 512;

pub struct Pipe {
    lock: SpinLock,
    data: [char; PIPESIZE],
    nread: u32,     // number of bytes read
    nwrite: u32,    // number of bytes written
    readopen: i32,  // read fd is still open
    writeopen: i32, // write fd is still open
}
