#![no_std]
#![no_main]
#![feature(asm)]
#![feature(global_asm)]
#![feature(naked_functions)]
#![feature(const_panic)]
#![feature(const_size_of_val)]

use core::mem::MaybeUninit;

use mmu::NPDENTRIES;
use param::MAXCPU;
use proc::Cpu;
use vm::PDE;

mod buf;
mod console;
mod file;
mod fs;
mod ide;
mod ioapic;
mod kalloc;
mod lapic;
mod log;
mod memlayout;
mod mmu;
mod mp;
mod param;
mod picirq;
mod pipe;
mod proc;
mod sleeplock;
mod spinlock;
mod switch;
mod trap;
mod trapasm;
mod trapvec;
mod uart;
mod vm;
mod x86;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[repr(transparent)]
struct StaticPointer<T>(*const T);

unsafe impl<T> Sync for StaticPointer<T> {}

#[repr(align(4096), C)]
struct Align4096<T>(T);

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

#[used]
#[no_mangle]
static ENTRYPGDIR: Align4096<[PDE; NPDENTRIES]> = entrypgdir();

#[used]
#[no_mangle]
static mut CPUS: MaybeUninit<[Cpu; MAXCPU]> = MaybeUninit::uninit();

#[used]
#[no_mangle]
static mut NCPU: usize = 0;

#[used]
#[no_mangle]
static mut IOAPICID: u8 = 0;

#[used]
#[no_mangle]
static mut LAPIC: *mut u32 = core::ptr::null_mut();

extern "C" {
    pub fn data();
}

// The boot page table used in entry.S and entryother.S.
// Page directories (and page tables) must start on page boundaries,
// hence the __aligned__ attribute.
// PTE_PS in a page directory entry enables 4Mbyte pages.
const fn entrypgdir() -> Align4096<[PDE; NPDENTRIES]> {
    use memlayout::KERNBASE;

    let mut pgdir = [PDE::NULL; NPDENTRIES];
    // Map VA's [0, 4MB) to PA's [0, 4MB)
    pgdir[0] = PDE::new(0, PDE::P | PDE::W | PDE::PS);
    // Map VA's [KERNBASE, KERNBASE+4MB) to PA's [0, 4MB)
    pgdir[KERNBASE >> 22] = PDE::new(0, PDE::P | PDE::W | PDE::PS);
    Align4096(pgdir)
}

// Bootstrap processor starts running C code here.
// Allocate a real stack and switch to it, first
// doing some setup required for memory allocator to work.
#[no_mangle]
unsafe extern "C" fn main() {
    use crate::ioapic::ioapicinit;
    use crate::lapic::lapicinit;
    use crate::memlayout::{p2v, PHYSTOP};
    use crate::mp::mp_init;
    use crate::picirq::picinit;
    use crate::uart::uartinit;
    use crate::vm::{kvm_alloc, seginit};

    extern "C" {
        fn end(); // first address after kernel loaded from ELF file
        fn kinit1(vstart: *const u8, vend: *const u8);

        fn consoleinit();
        fn pinit();
        fn tvinit();
        fn fileinit();
        fn ideinit();
        fn startothers();
        fn kinit2(vstart: *const u8, vend: *const u8);
        fn userinit();
        fn mpmain();
    }

    kinit1(end as _, p2v(4 * 1024 * 1024) as _); // phys page allocator
    kvm_alloc(); // kernel page table
    mp_init(); // detect other processors
    lapicinit(LAPIC); // interrupt controller
    seginit(); // segment descriptors
    picinit(); // disable pic
    ioapicinit(IOAPICID); // another interrupt controller
    consoleinit(); // console hardware
    uartinit(); // serial port
    pinit(); // process table
    tvinit(); // trap vectors
    fileinit(); // file table
    ideinit(); // disk
    startothers(); // start other processors
    kinit2(p2v(4 * 1024 * 1024) as _, p2v(PHYSTOP) as _); // must come after startothers()
    userinit(); // first user process
    mpmain(); // finish this processor's setup
}
