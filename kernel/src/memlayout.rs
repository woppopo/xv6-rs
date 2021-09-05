// Memory layout

pub const EXTMEM: usize = 0x100000; // Start of extended memory
pub const PHYSTOP: usize = 0xE000000; // Top physical memory
pub const DEVSPACE: usize = 0xFE000000; // Other devices are at high addresses

// Key addresses for address space layout (see kmap in vm.c for layout)
pub const KERNBASE: usize = 0x80000000; // First kernel virtual address
pub const KERNLINK: usize = KERNBASE + EXTMEM; // Address where kernel is linked

pub const fn p2v(paddr: usize) -> usize {
    paddr + KERNBASE
}

pub const fn v2p(vaddr: usize) -> usize {
    vaddr - KERNBASE
}
