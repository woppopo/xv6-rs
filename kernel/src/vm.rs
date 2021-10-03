use core::ffi::c_void;

use crate::{
    file::INode,
    fs::read_inode,
    kalloc::{kalloc, kalloc_zeroed, kfree},
    memlayout::{p2v, v2p, DEVSPACE, EXTMEM, KERNBASE, KERNLINK, PHYSTOP},
    mmu::{pg_address, pg_rounddown, pg_roundup, NPDENTRIES, PGSIZE},
    proc::Process,
    x86::lcr3,
};

#[repr(transparent)]
pub struct PDE(u32);

impl PDE {
    const NULL: Self = Self(0);

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
    const NULL: Self = Self(0);

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

// Load a program segment into pgdir.  addr must be page-aligned
// and the pages from addr to addr+sz must already be mapped.
fn uvm_load(pgdir: *mut PDE, addr: usize, ip: *const INode, offset: usize, size: usize) -> bool {
    if addr % PGSIZE != 0 {
        panic!("uvm_load: addr must be page aligned");
    }

    for i in (0..size).step_by(PGSIZE) {
        let pte =
            unsafe { walk_pgdir(pgdir, addr + i, false) }.expect("uvm_load: address should exist");

        let pa = unsafe { (*pte).address() };
        let n = if size - i < PGSIZE { size - i } else { PGSIZE };

        if read_inode(ip, p2v(pa), offset + i, n) != n {
            return false;
        }
    }

    true
}

// Allocate page tables and physical memory to grow process from oldsz to
// newsz, which need not be page aligned.  Returns new size or 0 on error.
fn uvm_alloc(pgdir: *mut PDE, size_old: usize, size_new: usize) -> usize {
    if size_new >= KERNBASE {
        return 0;
    }

    if size_new < size_old {
        return size_old;
    }

    extern "C" {
        fn deallocuvm(pgdir: *mut PDE, size_old: usize, size_new: usize) -> i32;
    }

    for a in (pg_roundup(size_old)..size_new).step_by(PGSIZE) {
        let mem = match kalloc_zeroed() {
            Some(mem) => mem,
            None => {
                //cprintf("allocuvm out of memory\n");
                unimplemented!();
                unsafe {
                    deallocuvm(pgdir, size_new, size_old);
                }
                return 0;
            }
        };

        if unsafe { !map_pages(pgdir, a, PGSIZE, v2p(mem), PTE::W | PTE::U) } {
            //cprintf("allocuvm out of memory (2)\n");
            unimplemented!();
            unsafe {
                deallocuvm(pgdir, size_new, size_old);
            }
            kfree(mem);
            return 0;
        }
    }

    size_new
}

// Deallocate user pages to bring the process size from oldsz to
// newsz.  oldsz and newsz need not be page-aligned, nor does newsz
// need to be less than oldsz.  oldsz can be larger than the actual
// process size.  Returns the new process size.
fn uvm_dealloc(pgdir: *mut PDE, size_old: usize, size_new: usize) -> usize {
    if size_new >= size_old {
        return size_old;
    }

    let mut a = pg_roundup(size_new);
    while a < size_old {
        let pte = unsafe { walk_pgdir(pgdir, a, false) };
        match pte {
            Some(pte) => {
                if unsafe { (*pte).is_present() } {
                    let pa = unsafe { (*pte).address() };
                    if pa == 0 {
                        panic!("kfree");
                    }
                    //kfree(p2v(pa));
                    unsafe {
                        *pte = PTE::NULL;
                    }
                }
                a += PGSIZE;
            }
            None => {
                a = pg_address(PDE::index(a) + 1, 0, 0);
            }
        }
    }

    size_new
}

// Free a page table and all the physical memory pages
// in the user part.
fn vm_free(pgdir: *mut PDE) {
    if pgdir.is_null() {
        panic!("vm_Free: no pgdir");
    }

    uvm_dealloc(pgdir, KERNBASE, 0);

    for i in 0..NPDENTRIES {
        if unsafe { (*pgdir.add(i)).is_present() } {
            kfree(p2v(unsafe { (*pgdir.add(i)).address() }));
        }
    }

    kfree(pgdir as usize);
}

// Clear PTE_U on a page. Used to create an inaccessible
// page beneath the user stack.
fn clear_pte_u(pgdir: *mut PDE, uva: usize) {
    unsafe {
        let pte = walk_pgdir(pgdir, uva, false).expect("clear_pte_u");
        (*pte).0 &= !PTE::U;
    }
}

// Given a parent process's page table, create a copy
// of it for a child.
fn uvm_copy(pgdir: *mut PDE, size: usize) -> Option<*mut PDE> {
    let dir = kvm_setup()?;
    for i in (0..size).step_by(PGSIZE) {
        let pte = unsafe { walk_pgdir(pgdir, i, false).expect("uvm_copy: pte should exist") };
        if unsafe { !(*pte).is_present() } {
            panic!("uvm_copy: page not present");
        }

        let pa = unsafe { (*pte).address() };
        let flags = unsafe { (*pte).flags() };

        let success = kalloc().and_then(|mem| {
            unsafe {
                core::ptr::copy_nonoverlapping(p2v(pa) as *const u8, mem as *mut u8, PGSIZE);
            }

            if unsafe { map_pages(dir, i, PGSIZE, v2p(mem), flags) } {
                Some(())
            } else {
                kfree(mem);
                None
            }
        });

        if success.is_none() {
            vm_free(pgdir);
            return None;
        }
    }

    Some(dir)
}

// Map user virtual address to kernel address.
fn uva_to_ka(pgdir: *mut PDE, uva: usize) -> Option<usize> {
    unsafe {
        let pte = walk_pgdir(pgdir, uva, false)
            .filter(|pte| (**pte).is_present() && (**pte).is_user())?;
        Some(p2v((*pte).address()))
    }
}

// Copy len bytes from p to user address va in page table pgdir.
// Most useful when pgdir is not the current page table.
// uva2ka ensures this only works for PTE_U pages.
fn copy_out(pgdir: *mut PDE, va: usize, p: usize, len: usize) -> bool {
    let mut len = len;
    let mut buf = p;
    let mut va = va;
    while len > 0 {
        let va0 = pg_rounddown(va);
        let pa0 = match uva_to_ka(pgdir, va0) {
            Some(pa2) => pa2,
            None => return false,
        };

        let n = (PGSIZE - (va - va0)).min(len);
        unsafe {
            core::ptr::copy_nonoverlapping((pa0 + (va - va0)) as *const u8, buf as *mut u8, n);
        }

        len -= n;
        buf += n;
        va = va0 + PGSIZE;
    }

    true
}

mod _binding {
    use super::*;

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
        match map {
            true => 0,
            false => -1,
        }
    }

    #[no_mangle]
    extern "C" fn loaduvm(
        pgdir: *mut PDE,
        dst: *mut u8,
        ip: *const INode,
        offset: u32,
        size: u32,
    ) -> i32 {
        let load = uvm_load(pgdir, dst as usize, ip, offset as usize, size as usize);
        match load {
            true => 0,
            false => -1,
        }
    }

    #[no_mangle]
    extern "C" fn allocuvm(pgdir: *mut PDE, oldsz: u32, newsz: u32) -> i32 {
        uvm_alloc(pgdir, oldsz as usize, newsz as usize) as i32
    }

    #[no_mangle]
    extern "C" fn deallocuvm(pgdir: *mut PDE, oldsz: u32, newsz: u32) -> i32 {
        uvm_dealloc(pgdir, oldsz as usize, newsz as usize) as i32
    }

    #[no_mangle]
    extern "C" fn freevm(pgdir: *mut PDE) {
        vm_free(pgdir);
    }

    #[no_mangle]
    extern "C" fn clearpteu(pgdir: *mut PDE, uva: *const i8) {
        clear_pte_u(pgdir, uva as usize);
    }

    #[no_mangle]
    extern "C" fn copyuvm(pgdir: *mut PDE, sz: u32) -> *mut PDE {
        let pde = uvm_copy(pgdir, sz as usize);
        match pde {
            Some(pde) => pde,
            None => core::ptr::null_mut(),
        }
    }

    #[no_mangle]
    extern "C" fn uva2ka(pgdir: *mut PDE, uva: *const i8) -> *const i8 {
        match uva_to_ka(pgdir, uva as usize) {
            Some(addr) => addr as *const i8,
            None => core::ptr::null(),
        }
    }

    #[no_mangle]
    extern "C" fn copyout(pgdir: *mut PDE, va: u32, p: *const c_void, len: u32) -> i32 {
        match copy_out(pgdir, va as usize, p as usize, len as usize) {
            true => 0,
            false => -1,
        }
    }
}
