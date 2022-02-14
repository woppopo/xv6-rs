#![allow(incomplete_features)]
#![no_std]
#![no_main]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(once_cell)]
#![feature(const_size_of_val)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(inline_const)]
#![feature(inline_const_pat)]
#![feature(let_else)]

use core::mem::MaybeUninit;

use mmu::NPDENTRIES;
use param::MAXCPU;
use proc::Cpu;
use sync_hack::SyncHack;
use vm::PDE;

mod buf;
mod console;
mod elf;
mod file;
mod fs;
mod ide;
mod interrupt;
mod ioapic;
mod kalloc;
mod keyboard;
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
mod string;
mod switch;
mod sync_hack;
mod syscall;
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

#[repr(align(4096), C)]
struct Align4096<T>(T);

impl<T> core::ops::Deref for Align4096<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

static INITCODE: &'static [u8] = include_bytes!(concat!(env!("OUT_DIR"), "/initcode"));

#[used]
#[no_mangle]
static _binary_initcode_start: SyncHack<*const u8> = SyncHack(INITCODE.as_ptr());

#[used]
#[no_mangle]
static _binary_initcode_size: usize = ENTRYOTHER.len();

static ENTRYOTHER: &'static [u8] = include_bytes!(concat!(env!("OUT_DIR"), "/entryother"));

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

static mut LAPIC_ADDRESS: *mut u32 = core::ptr::null_mut();

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
    use crate::ide::init_ide;
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
        fn fileinit();
        fn kinit2(vstart: *const u8, vend: *const u8);
        fn userinit();
    }

    kinit1(end as _, p2v(4 * 1024 * 1024) as _); // phys page allocator
    kvm_alloc(); // kernel page table
    mp_init(); // detect other processors
    lapicinit(LAPIC_ADDRESS); // interrupt controller
    seginit(); // segment descriptors
    picinit(); // disable pic
    ioapicinit(IOAPICID); // another interrupt controller
    consoleinit(); // console hardware
    uartinit(); // serial port
    pinit(); // process table
    fileinit(); // file table
    init_ide(NCPU); // disk
    startothers(); // start other processors
    kinit2(p2v(4 * 1024 * 1024) as _, p2v(PHYSTOP) as _); // must come after startothers()
    userinit(); // first user process
    mp_main(); // finish this processor's setup
}

// Common CPU setup code.
unsafe fn mp_main() {
    use crate::proc::my_cpu;
    use crate::trap::load_interrupt_descriptor_table;

    extern "C" {
        fn scheduler();
    }

    //cprintf("cpu%d: starting %d\n", cpuid(), cpuid());
    load_interrupt_descriptor_table(); // load idt register

    // tell startothers() we're up
    my_cpu()
        .started
        .store(1, core::sync::atomic::Ordering::SeqCst);

    scheduler(); // start running processes
}

// Other CPUs jump here from entryother.S.
extern "C" fn mp_enter() {
    use crate::lapic::lapicinit;
    use crate::vm::{kvm_switch, seginit};

    kvm_switch();
    seginit();
    lapicinit(unsafe { LAPIC_ADDRESS });
    unsafe {
        mp_main();
    }
}

// Start the non-boot (AP) processors.
fn startothers() {
    use crate::kalloc::kalloc;
    use crate::memlayout::{p2v, v2p};
    use crate::param::KSTACKSIZE;
    use crate::proc::my_cpu;

    // Write entry code to unused memory at 0x7000.
    // The linker has placed the image of entryother.S in
    // _binary_entryother_start.
    let code = p2v(0x7000) as *mut u8;
    unsafe {
        ENTRYOTHER
            .as_ptr()
            .copy_to_nonoverlapping(code, ENTRYOTHER.len());
    }

    let cpus = unsafe { CPUS.assume_init_ref() };
    let ncpu = unsafe { NCPU };
    for c in cpus.iter().take(ncpu) {
        // We've started already.
        if (c as *const _) == my_cpu() {
            continue;
        }

        // Tell entryother.S what stack to use, where to enter, and what
        // pgdir to use. We cannot use kpgdir yet, because the AP processor
        // is running in low  memory, so we use ENTRYPGDIR for the APs too.
        let stack = kalloc().unwrap();
        unsafe {
            *code.sub(4).cast::<usize>() = stack + KSTACKSIZE;
            *code.sub(8).cast::<extern "C" fn()>() = mp_enter;
            *code.sub(12).cast::<usize>() = v2p(ENTRYPGDIR.as_ptr() as usize);
        }
        crate::lapic::lapicstartap(c.apicid, v2p(code as usize) as u32);

        while c.started.load(core::sync::atomic::Ordering::SeqCst) == 0 {}
    }
}
