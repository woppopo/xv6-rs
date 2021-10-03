use core::ffi::c_void;

// Page directory and page table constants.
pub const NPDENTRIES: usize = 1024; // # directory entries per page directory
pub const NPTENTRIES: usize = 1024; // # PTEs per page table
pub const PGSIZE: usize = 4096; // bytes mapped by a page

// cpu->gdt[NSEGS] holds the above segments.
pub const NSEGS: usize = 6;

#[repr(C)]
pub struct SegmentDescriptorTable {
    pub _null: SegmentDescriptor,
    pub kernel_code: SegmentDescriptor,
    pub kernel_data: SegmentDescriptor,
    pub user_code: SegmentDescriptor,
    pub user_data: SegmentDescriptor,
    pub task_state: SegmentDescriptor,
}

impl SegmentDescriptorTable {
    pub fn load(&self) {
        #[repr(C, packed)]
        struct GDTR {
            limit: u16,
            base: *const SegmentDescriptorTable,
        }

        let gdtr = GDTR {
            limit: (core::mem::size_of::<Self>() - 1) as u16,
            base: self,
        };

        unsafe {
            asm!("lgdt [{0}]", in(reg) &gdtr);
        }
    }
}

#[repr(C, packed)]
pub struct SegmentDescriptor(u32, u32);

impl SegmentDescriptor {
    pub const fn null() -> Self {
        Self(0, 0)
    }

    pub const fn new(ty: u8, base: u32, limit: u32, dpl: u8) -> Self {
        const fn bits(a: u32, start: usize, end: usize) -> u32 {
            let len = end - start;
            let mask = (1 << (len + 1)) - 1;
            (a >> start) & mask
        }

        let desc0 = bits(base, 0, 15) << 16 | bits(limit, 0, 15);
        let desc1 = bits(base, 24, 31) << 24
            | 1 << 23
            | 1 << 22
            | 0 << 21
            | 0 << 20
            | bits(limit, 16, 19) << 16
            | 1 << 15
            | (dpl as u32) << 13
            | 1 << 12
            | (ty as u32) << 8
            | bits(base, 16, 23);

        Self(desc0, desc1)
    }

    pub const fn new16(ty: u8, base: u32, limit: u32, dpl: u8, s: bool) -> Self {
        todo!()
    }
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
    ss2: u16,
    padding3: u16,
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
