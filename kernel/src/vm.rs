use core::ffi::c_void;

use crate::{kalloc::kalloc_zeroed, memlayout::p2v};

#[repr(transparent)]
struct PDE(u32);

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

// Return the address of the PTE in page table pgdir
// that corresponds to virtual address va.  If alloc!=0,
// create any required page table pages.
unsafe fn walk_pgdir(pgdir: *mut PDE, va: *const c_void, alloc: u32) -> *const PTE {
    let pde = pgdir.add(PDE::index(va as usize));
    let pgtab = if (*pde).is_present() {
        p2v((*pde).address())
    } else {
        if alloc == 0 {
            return core::ptr::null();
        }

        let pgtab = match kalloc_zeroed() {
            Some(addr) => addr,
            None => return core::ptr::null(),
        };

        *pde = PDE::new(pgtab, PDE::P | PDE::W | PDE::U);
        pgtab
    };

    (pgtab as *const PTE).add(PTE::index(va as usize))
}
