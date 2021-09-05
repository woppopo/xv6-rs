#![no_std]
#![no_main]
#![feature(asm)]
#![feature(global_asm)]
#![feature(naked_functions)]

mod console;
mod ioapic;
mod lapic;
mod memlayout;
mod picirq;
mod switch;
mod trap;
mod trapasm;
mod trapvec;
mod uart;
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
    use crate::ioapic::ioapicinit;
    use crate::lapic::lapicinit;
    use crate::memlayout::{p2v, PHYSTOP};
    use crate::picirq::picinit;
    use crate::uart::uartinit;

    extern "C" {
        static ioapicid: u8;
        static lapic: *mut u32;
        fn end(); // first address after kernel loaded from ELF file
        fn kinit1(vstart: *const u8, vend: *const u8);
        fn kvmalloc();
        fn mpinit();
        fn seginit();
        fn consoleinit();
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

    kinit1(end as _, p2v(4 * 1024 * 1024) as _); // phys page allocator
    kvmalloc(); // kernel page table
    mpinit(); // detect other processors
    lapicinit(lapic); // interrupt controller
    seginit(); // segment descriptors
    picinit(); // disable pic
    ioapicinit(ioapicid); // another interrupt controller
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
