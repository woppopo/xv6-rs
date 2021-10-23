// This file contains definitions for the
// x86 memory management unit (MMU).

use core::ffi::c_void;

// Eflags register
pub const FL_IF: u32 = 0x00000200; // Interrupt Enable

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
    pub const KERNEL_CODE_SELECTOR: u16 = 1 << 3;
    pub const KERNEL_DATA_SELECTOR: u16 = 2 << 3;
    pub const USER_CODE_SELECTOR: u16 = 3 << 3;
    pub const USER_DATA_SELECTOR: u16 = 4 << 3;
    pub const TASK_STATE_SELECTOR: u16 = 5 << 3;

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

    pub const fn new32(ty: u8, base: u32, limit: u32, dpl: u8) -> Self {
        let desc0 = (base & 0xffff) << 16 | ((limit >> 12) & 0xffff);
        let desc1 = (base >> 24) << 24
            | 1 << 23
            | 1 << 22
            | 0 << 21
            | 0 << 20
            | (limit >> 28) << 16
            | 1 << 15
            | (dpl as u32) << 13
            | 1 << 12
            | (ty as u32) << 8
            | (base >> 16) & 0xff;

        Self(desc0, desc1)
    }

    pub const fn new16(ty: u8, base: u32, limit: u32, dpl: u8, s: bool) -> Self {
        let desc0 = (base & 0xffff) << 16 | (limit & 0xffff);
        let desc1 = (base >> 24) << 24
            | 0 << 23
            | 1 << 22
            | 0 << 21
            | 0 << 20
            | (limit >> 16) << 16
            | 1 << 15
            | (dpl as u32) << 13
            | if s { 1 } else { 0 } << 12
            | (ty as u32) << 8
            | (base >> 16) & 0xff;

        Self(desc0, desc1)
    }
}

// Task state segment format
#[repr(C)]
pub struct TaskState {
    pub link: u32, // Old ts selector
    pub esp0: u32, // Stack pointers and segment selectors
    pub ss0: u16,  //   after an increase in privilege level
    pub padding1: u16,
    pub esp1: *const u32,
    pub ss1: u16,
    pub padding2: u16,
    pub esp2: *const u32,
    pub ss2: u16,
    pub padding3: u16,
    pub cr3: *const c_void, // Page directory base
    pub eip: *const u32,    // Saved state from last task switch
    pub eflags: u32,
    pub eax: u32, // More saved state (registers)
    pub ecx: u32,
    pub edx: u32,
    pub ebx: u32,
    pub esp: *const u32,
    pub ebp: *const u32,
    pub esi: u32,
    pub edi: u32,
    pub es: u16, // Even more saved state (segment selectors)
    pub padding4: u16,
    pub cs: u16,
    pub padding5: u16,
    pub ss: u16,
    pub padding6: u16,
    pub ds: u16,
    pub padding7: u16,
    pub fs: u16,
    pub padding8: u16,
    pub gs: u16,
    pub padding9: u16,
    pub ldt: u16,
    pub padding10: u16,
    pub t: u16,    // Trap on task switch
    pub iomb: u16, // I/O map base address
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
