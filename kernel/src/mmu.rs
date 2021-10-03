use core::ffi::c_void;

// Page directory and page table constants.
pub const NPDENTRIES: usize = 1024; // # directory entries per page directory
pub const NPTENTRIES: usize = 1024; // # PTEs per page table
pub const PGSIZE: usize = 4096; // bytes mapped by a page

// cpu->gdt[NSEGS] holds the above segments.
pub const NSEGS: usize = 6;

#[repr(C)]
pub struct SegmentDescriptor {
    lim_15_0: u16,  // Low bits of segment limit
    base_15_0: u16, // Low bits of segment base address
    base_23_16: u8, // Middle bits of segment base address
    /*
    type: u4; // Segment type (see STS_ constants)
    s: u1; // 0 = system, 1 = application
    dpl: u2; // Descriptor Privilege Level
    p: u1; // Present
    */
    type_s_dpl_p: u8,
    /*
    lim_19_16: u4; // High bits of segment limit
    avl: u1; // Unused (available for software use)
    rsv1: u1; // Reserved
    db: u1; // 0 = 16-bit segment, 1 = 32-bit segment
    g: u1; // Granularity: limit scaled by 4K when set
    */
    lim_19_16_avl_rsv1_db_g: u8,
    base_31_24: u8, // High bits of segment base address
}

// Task state segment format
#[repr(C)]
pub struct TaskState {
    link: u32, // Old ts selector
    esp0: u32, // Stack pointers and segment selectors
    ss0: u16,  //   after an increase in privilege level
    padding1: u16,
    esp1: *const u32,
    ss1: u16,
    padding2: u16,
    esp2: *const u32,
    ss2: u32,
    padding3: u32,
    cr3: *const c_void, // Page directory base
    eip: *const u32,    // Saved state from last task switch
    eflags: u32,
    eax: u32, // More saved state (registers)
    ecx: u32,
    edx: u32,
    ebx: u32,
    esp: *const u32,
    ebp: *const u32,
    esi: u32,
    edi: u32,
    es: u16, // Even more saved state (segment selectors)
    padding4: u16,
    cs: u16,
    padding5: u16,
    ss: u16,
    padding6: u16,
    ds: u16,
    padding7: u16,
    fs: u16,
    padding8: u16,
    gs: u16,
    padding9: u16,
    ldt: u16,
    padding10: u16,
    t: u16,    // Trap on task switch
    iomb: u16, // I/O map base address
}

pub const fn pg_roundup(size: usize) -> usize {
    (size + PGSIZE - 1) & !(PGSIZE - 1)
}

pub const fn pg_rounddown(size: usize) -> usize {
    size & !(PGSIZE - 1)
}

// construct virtual address from indexes and offset
pub const fn pg_address(tbl_id: usize, entry_id: usize, offset: usize) -> usize {
    (tbl_id << 22) | (entry_id << 12) | offset
}
