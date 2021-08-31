#![no_std]
#![no_main]
#![feature(asm)]
#![feature(global_asm)]

mod picirq;
mod switch;
mod x86;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[repr(transparent)]
struct StaticPointer<T>(*const T);

unsafe impl<T> Sync for StaticPointer<T> {}

#[used]
#[no_mangle]
static INITCODE: &'static [u8] = include_bytes!(concat!(env!("OUT_DIR"), "/initcode"));

#[used]
#[no_mangle]
static _binary_initcode_start: StaticPointer<u8> = StaticPointer(INITCODE.as_ptr());

#[used]
#[no_mangle]
static _binary_initcode_size: usize = INITCODE.len();

#[used]
#[no_mangle]
static ENTRYOTHER: &'static [u8] = include_bytes!(concat!(env!("OUT_DIR"), "/entryother"));

#[used]
#[no_mangle]
static _binary_entryother_start: StaticPointer<u8> = StaticPointer(ENTRYOTHER.as_ptr());

#[used]
#[no_mangle]
static _binary_entryother_size: usize = ENTRYOTHER.len();

// Bootstrap processor starts running C code here.
// Allocate a real stack and switch to it, first
// doing some setup required for memory allocator to work.
#[no_mangle]
unsafe extern "C" fn main() {
    use crate::picirq::picinit;

    pub const PHYSTOP: usize = 0xE000000; // Top physical memory

    extern "C" {
        fn end(); // first address after kernel loaded from ELF file
        fn kinit1(vstart: *const u8, vend: *const u8);
        fn kvmalloc();
        fn mpinit();
        fn lapicinit();
        fn seginit();
        fn ioapicinit();
        fn consoleinit();
        fn uartinit();
        fn pinit();
        fn tvinit();
        fn binit();
        fn fileinit();
        fn ideinit();
        fn startothers();
        fn kinit2(vstart: *const u8, vend: *const u8);
        fn userinit();
        fn mpmain();
    }

    pub const KERNBASE: usize = 0x80000000; // First kernel virtual address
    fn p2v(paddr: usize) -> usize {
        paddr + KERNBASE
    }

    fn v2p(vaddr: usize) -> usize {
        vaddr - KERNBASE
    }

    kinit1(end as _, p2v(4 * 1024 * 1024) as _); // phys page allocator
    kvmalloc(); // kernel page table
    mpinit(); // detect other processors
    lapicinit(); // interrupt controller
    seginit(); // segment descriptors
    picinit(); // disable pic
    ioapicinit(); // another interrupt controller
    consoleinit(); // console hardware
    uartinit(); // serial port
    pinit(); // process table
    tvinit(); // trap vectors
    binit(); // buffer cache
    fileinit(); // file table
    ideinit(); // disk
    startothers(); // start other processors
    kinit2(p2v(4 * 1024 * 1024) as _, p2v(PHYSTOP) as _); // must come after startothers()
    userinit(); // first user process
    mpmain(); // finish this processor's setup
}
