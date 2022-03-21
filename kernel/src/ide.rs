use crate::{
    buf::Buffer,
    fs::BSIZE,
    ioapic::ioapicenable,
    param::FSSIZE,
    proc::{sleep, wakeup},
    spinlock::SpinLockC,
    trap::IRQ_IDE,
    x86::{inb, insl, outb, outsl},
};

const SECTOR_SIZE: usize = 512;
const IDE_BSY: u8 = 0x80;
const IDE_DRDY: u8 = 0x40;
const IDE_DF: u8 = 0x20;
const IDE_ERR: u8 = 0x01;
const IDE_CMD_READ: u8 = 0x20;
const IDE_CMD_WRITE: u8 = 0x30;
const IDE_CMD_RDMUL: u8 = 0xc4;
const IDE_CMD_WRMUL: u8 = 0xc5;

// idequeue points to the buf now being read/written to the disk.
// idequeue->qnext points to the next buf to be processed.
// You must hold idelock while manipulating queue.

pub static mut IDE: Option<IDE> = None;

pub struct IDE {
    lock: SpinLockC,
    queue: *mut Buffer,
    havedisk1: bool,
}

impl IDE {
    pub fn new() -> Self {
        wait_ide();

        // Check if disk 1 is present
        unsafe {
            outb(0x1f6, 0xe0 | (1 << 4));
        }

        let mut havedisk1 = false;
        for _i in 0..1000 {
            if unsafe { inb(0x1f7) != 0 } {
                havedisk1 = true;
                break;
            }
        }

        // Switch back to disk 0.
        unsafe {
            outb(0x1f6, 0xe0 | (0 << 4));
        }

        Self {
            lock: SpinLockC::new(),
            queue: core::ptr::null_mut(),
            havedisk1,
        }
    }

    // Start the request for b.  Caller must hold idelock.
    pub fn start(&mut self, buf: *mut Buffer) {
        if buf.is_null() {
            panic!("IDE::start");
        }

        if unsafe { (*buf).blockno >= FSSIZE } {
            panic!("incorrect blockno");
        }

        let sector_per_block = BSIZE / SECTOR_SIZE;
        let sector = unsafe { (*buf).blockno * sector_per_block as usize };
        let read_cmd = if sector_per_block == 1 {
            IDE_CMD_READ
        } else {
            IDE_CMD_RDMUL
        };
        let write_cmd = if sector_per_block == 1 {
            IDE_CMD_WRITE
        } else {
            IDE_CMD_WRMUL
        };

        if sector_per_block > 7 {
            panic!("IDE::start");
        }

        wait_ide();
        unsafe {
            outb(0x3f6, 0); // generate interrupt
            outb(0x1f2, sector_per_block as u8); // number of sectors
            outb(0x1f3, (sector & 0xff) as u8);
            outb(0x1f4, ((sector >> 8) & 0xff) as u8);
            outb(0x1f5, ((sector >> 16) & 0xff) as u8);
            outb(
                0x1f6,
                (0xe0 | (((*buf).dev & 1) << 4) | ((sector >> 24) & 0x0f)) as u8,
            );

            if (*buf).flags & Buffer::DIRTY != 0 {
                outb(0x1f7, write_cmd);
                outsl(0x1f0, (*buf).data.as_ptr() as *const u32, BSIZE / 4);
            } else {
                outb(0x1f7, read_cmd);
            }
        }
    }

    pub fn interrupt_handler(&mut self) {
        // First queued buffer is the active request.
        self.lock.acquire();

        let buf = self.queue;
        if buf.is_null() {
            self.lock.release();
            return;
        }

        self.queue = unsafe { (*buf).qnext };

        // Read data if needed.
        unsafe {
            if (*buf).flags & Buffer::DIRTY == 0 && wait_ide() {
                insl(0x1f0, (*buf).data.as_ptr() as *mut u32, BSIZE / 4);
            }
        }

        // Wake process waiting for this buf.
        unsafe {
            (*buf).flags |= Buffer::VALID;
            (*buf).flags &= !Buffer::DIRTY;
            wakeup(buf as *const _);
        }

        if !self.queue.is_null() {
            self.start(self.queue);
        }

        self.lock.release();
    }

    // Sync buf with disk.
    // If B_DIRTY is set, write buf to disk, clear B_DIRTY, set B_VALID.
    // Else if B_VALID is not set, read buf from disk, set B_VALID.
    pub fn rw(&mut self, buf: *mut Buffer) {
        if unsafe { !(*buf).lock.is_locked() } {
            panic!("IDE::rw: buf not locked");
        }

        if unsafe { (*buf).flags & (Buffer::VALID | Buffer::DIRTY) } == Buffer::VALID {
            panic!("IDE::rw: nothing to do");
        }

        if unsafe { (*buf).dev != 0 } && !self.havedisk1 {
            panic!("IDE::rw: ide disk 1 not present");
        }

        self.lock.acquire(); // DOC: acquire-lock

        // Append buf to idequeue.
        unsafe {
            (*buf).qnext = core::ptr::null_mut();
            let mut pp = &mut self.queue;
            while !(*pp).is_null() {
                pp = &mut (**pp).qnext;
            }
            *pp = buf;
        }

        // Start disk if necessary.
        if self.queue == buf {
            self.start(buf);
        }

        // Wait for request to finish.
        unsafe {
            while (*buf).flags & (Buffer::VALID | Buffer::DIRTY) != Buffer::VALID {
                sleep(buf as *const _, &self.lock);
            }
        }

        self.lock.release();
    }
}

// Wait for IDE disk to become ready.
fn wait_ide() -> bool {
    let mut status;
    while {
        status = unsafe { inb(0x1f7) };
        status & (IDE_BSY | IDE_DRDY) != IDE_DRDY
    } {}

    status & (IDE_DF | IDE_ERR) == 0
}

pub fn init_ide(ncpu: usize) {
    ioapicenable(IRQ_IDE, (ncpu - 1) as u32);
    unsafe {
        IDE = Some(IDE::new());
    }
}

pub fn ide_interrupt() {
    let ide = unsafe { IDE.as_mut().unwrap() };
    ide.interrupt_handler();
}

mod _binding {
    use super::*;

    #[no_mangle]
    unsafe extern "C" fn iderw(b: *mut Buffer) {
        let ide = IDE.as_mut().unwrap();
        ide.rw(b);
    }
}
