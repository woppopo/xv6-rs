use core::ffi::c_void;

use crate::{
    kalloc::kalloc_zeroed,
    memlayout::{p2v, v2p, DEVSPACE, EXTMEM, KERNBASE, KERNLINK, PHYSTOP},
    mmu::{pg_rounddown, PGSIZE},
    x86::lcr3,
};

#[repr(transparent)]
pub struct PDE(u32);

impl PDE {
    const P: u32 = 0x001;
    const W: u32 = 0x002;
    const U: u32 = 0x004;
    const PS: u32 = 0x080;

    pub const fn new(addr: usize, flags: u32) -> Self {
        assert!(core::mem::size_of_val(&addr) == core::mem::size_of::<u32>());

        let addr = (addr as u32) & !0xfff;
        let flags = flags & 0xfff;
        Self(addr | flags)
    }

    pub const fn is_present(&self) -> bool {
        self.0 & Self::P != 0
    }

    pub const fn is_writable(&self) -> bool {
        self.0 & Self::W != 0
    }

    pub const fn is_user(&self) -> bool {
        self.0 & Self::U != 0
    }

    pub const fn is_pagesize(&self) -> bool {
        self.0 & Self::PS != 0
    }

    pub const fn address(&self) -> usize {
        (self.0 & !0xfff) as usize
    }

    pub const fn flags(&self) -> u32 {
        self.0 & 0xfff
    }

    pub const fn index(va: usize) -> usize {
        (va >> 22) & 0x3ff
    }
}

#[repr(transparent)]
struct PTE(u32);

impl PTE {
    const P: u32 = 0x001;
    const W: u32 = 0x002;
    const U: u32 = 0x004;
    const PS: u32 = 0x080;

    pub const fn new(addr: usize, flags: u32) -> Self {
        assert!(core::mem::size_of_val(&addr) == core::mem::size_of::<u32>());

        let addr = (addr as u32) & !0xfff;
        let flags = flags & 0xfff;
        Self(addr | flags)
    }

    pub const fn is_present(&self) -> bool {
        self.0 & Self::P != 0
    }

    pub const fn is_writable(&self) -> bool {
        self.0 & Self::W != 0
    }

    pub const fn is_user(&self) -> bool {
        self.0 & Self::U != 0
    }

    pub const fn is_pagesize(&self) -> bool {
        self.0 & Self::PS != 0
    }

    pub const fn address(&self) -> usize {
        (self.0 & !0xfff) as usize
    }

    pub const fn flags(&self) -> u32 {
        self.0 & 0xfff
    }

    pub const fn index(va: usize) -> usize {
        (va >> 12) & 0x3ff
    }
}

#[no_mangle]
extern "C" fn walkpgdir(pgdir: *mut PDE, va: *const c_void, alloc: u32) -> *mut PTE {
    unsafe { walk_pgdir(pgdir, va as usize, alloc != 0) }.unwrap_or(core::ptr::null_mut())
}

#[no_mangle]
extern "C" fn setupkvm() -> *mut PDE {
    kvm_setup().unwrap_or(core::ptr::null_mut())
}

#[no_mangle]
extern "C" fn kvmalloc() {
    kvm_alloc()
}

#[no_mangle]
extern "C" fn switchkvm() {
    kvm_switch()
}

#[no_mangle]
extern "C" fn inituvm(pgdir: *mut PDE, init: *const u8, sz: u32) {
    uvm_init(pgdir, unsafe {
        core::slice::from_raw_parts(init, sz as usize)
    });
}

#[no_mangle]
extern "C" fn mappages(pde: *mut PDE, va: *const c_void, size: u32, pa: u32, perm: u32) -> i32 {
    let map = unsafe { map_pages(pde, va as usize, size as usize, pa as usize, perm) };
    if map {
        0
    } else {
        -1
    }
}

// Return the address of the PTE in page table pgdir
// that corresponds to virtual address va.  If alloc!=0,
// create any required page table pages.
unsafe fn walk_pgdir(pgdir: *mut PDE, va: usize, alloc: bool) -> Option<*mut PTE> {
    let pde = pgdir.add(PDE::index(va as usize));
    let pgtab = if (*pde).is_present() {
        p2v((*pde).address())
    } else {
        if !alloc {
            return None;
        }

        let pgtab = kalloc_zeroed()?;
        *pde = PDE::new(v2p(pgtab), PDE::P | PDE::W | PDE::U);
        pgtab
    };

    let pt = (pgtab as *mut PTE).add(PTE::index(va as usize));
    Some(pt)
}

// Create PTEs for virtual addresses starting at va that refer to
// physical addresses starting at pa. va and size might not
// be page-aligned.
unsafe fn map_pages(pgdir: *mut PDE, va: usize, size: usize, mut pa: usize, perm: u32) -> bool {
    let mut a = pg_rounddown(va);
    let last = pg_rounddown(va + size - 1);

    loop {
        let pte = match walk_pgdir(pgdir, a, true) {
            Some(pte) => pte,
            None => return false,
        };

        if (*pte).is_present() {
            panic!("remap");
        }
        *pte = PTE::new(pa, perm | PTE::P);

        if a == last {
            break;
        }

        a += PGSIZE;
        pa += PGSIZE;
    }

    true
}

struct KMap {
    virt: usize,
    phys_start: usize,
    phys_end: usize,
    perm: u32,
}

// There is one page table per process, plus one that's used when
// a CPU is not running any process (kpgdir). The kernel uses the
// current process's page table during system calls and interrupts;
// page protection bits prevent user code from using the kernel's
// mappings.
//
// setupkvm() and exec() set up every page table like this:
//
//   0..KERNBASE: user memory (text+data+stack+heap), mapped to
//                phys memory allocated by the kernel
//   KERNBASE..KERNBASE+EXTMEM: mapped to 0..EXTMEM (for I/O space)
//   KERNBASE+EXTMEM..data: mapped to EXTMEM..V2P(data)
//                for the kernel's instructions and r/o data
//   data..KERNBASE+PHYSTOP: mapped to V2P(data)..PHYSTOP,
//                                  rw data + free physical memory
//   0xfe000000..0: mapped direct (devices such as ioapic)
//
// The kernel allocates physical memory for its heap and for user memory
// between V2P(end) and the end of physical memory (PHYSTOP)
// (directly addressable from end..P2V(PHYSTOP)).

fn kvm_setup() -> Option<*mut PDE> {
    let pgdir = kalloc_zeroed()?;
    let pgdir = pgdir as *mut PDE;

    if p2v(PHYSTOP) > DEVSPACE {
        panic!("PHYSTOP too high");
    }

    extern "C" {
        fn data();
    }

    // This table defines the kernel's mappings, which are present in
    // every process's page table.
    let kmap = [
        // I/O space
        KMap {
            virt: KERNBASE,
            phys_start: 0,
            phys_end: EXTMEM,
            perm: PTE::W,
        },
        // kern text+rodata
        KMap {
            virt: KERNLINK,
            phys_start: v2p(KERNLINK),
            phys_end: v2p(data as usize),
            perm: 0,
        },
        // kern data+memory
        KMap {
            virt: data as usize,
            phys_start: v2p(data as usize),
            phys_end: PHYSTOP,
            perm: PTE::W,
        },
        // more devices
        KMap {
            virt: DEVSPACE,
            phys_start: DEVSPACE,
            phys_end: usize::MAX,
            perm: PTE::W,
        },
    ];

    for k in kmap {
        if unsafe {
            !map_pages(
                pgdir,
                k.virt,
                k.phys_end - k.phys_start,
                k.phys_start,
                k.perm,
            )
        } {
            unsafe {
                extern "C" {
                    fn freevm(pgdir: *mut PDE);
                }
                freevm(pgdir);
            }
            return None;
        }
    }

    Some(pgdir)
}

extern "C" {
    static mut kpgdir: *mut PDE;
}

// Allocate one page table for the machine for the kernel address
// space for scheduler processes.
pub fn kvm_alloc() {
    unsafe {
        kpgdir = kvm_setup().expect("");
    }
    kvm_switch();
}

// Switch h/w page table register to the kernel-only page table,
// for when no process is running.
fn kvm_switch() {
    unsafe {
        lcr3(v2p(kpgdir as usize) as u32); // switch to the kernel page table
    }
}

// Load the initcode into address 0 of pgdir.
// sz must be less than a page.
fn uvm_init(pgdir: *mut PDE, init: &[u8]) {
    if init.len() > PGSIZE {
        panic!("uvm_init: more than a page");
    }

    let mem = kalloc_zeroed().expect("oom");
    unsafe {
        map_pages(pgdir, 0, PGSIZE, v2p(mem), PTE::W | PTE::U);
        core::ptr::copy_nonoverlapping(init.as_ptr(), mem as *mut u8, init.len());
    }
}
