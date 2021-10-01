use core::ffi::c_void;

use crate::{
    kalloc::kalloc_zeroed,
    memlayout::{p2v, v2p},
    mmu::{pg_rounddown, PGSIZE},
};

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

#[no_mangle]
extern "C" fn walkpgdir(pgdir: *mut PDE, va: *const c_void, alloc: u32) -> *mut PTE {
    unsafe { walk_pgdir(pgdir, va as usize, alloc != 0) }.unwrap_or(core::ptr::null_mut())
}

#[no_mangle]
unsafe extern "C" fn mappages(
    pde: *mut PDE,
    va: *const c_void,
    size: u32,
    pa: u32,
    perm: u32,
) -> i32 {
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
