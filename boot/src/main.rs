#![no_std]
#![no_main]
#![feature(asm_const)]
#![feature(const_size_of_val)]
// Intel記法では何故かljmpのオペランドにCSレジスタの値を書けないので、
// 一時的にAT&T記法を使用する
// その際に発生する警告を無効化する
#![allow(bad_asm_style)]

use core::arch::{asm, global_asm};

// 以下の定数値はglobal_asmで使用されているが、
// 何故か未使用の警告が出るのでそれを無効化する
#[allow(unused)]
const CR0_PROTECTION_ENABLE: u32 = 0x00000001;
#[allow(unused)]
const SEGMENT_KERNEL_CODE: u32 = 1;
#[allow(unused)]
const SEGMENT_KERNEL_DATA: u32 = 2;

const SECTOR_SIZE: usize = 512;

global_asm!(
    r#"
    .code16
    .globl start
    start:
        cli
        xor ax, ax # ax = 0
        mov ds, ax # データセグメント = 0
        mov es, ax # エクストラセグメント = 0
        mov ss, ax # スタックセグメント = 0

    # Physical address line A20 is tied to zero so that the first PCs
    # with 2 MB would run software that assumed 1 MB.  Undo that.
    set_a20_1:
        in al, 0x64  # Wait for not busy
        test al, 0x2
        jnz set_a20_1

        mov al, 0xd1 # 0xd1 -> port 0x64
        out 0x64, al

    set_a20_2:
        in al, 0x64 # Wait for not busy
        test al, 0x2
        jnz set_a20_2

        mov al, 0xdf # 0xdf -> port 0x60
        out 0x60, al

    # Switch from real to protected mode.  Use a bootstrap GDT that makes
    # virtual addresses map directly to physical addresses so that the
    # effective memory map doesn't change during the transition.
    lgdt GDT_DESC
    mov eax, cr0
    or eax, {cr0}
    mov cr0, eax

    # Complete the transition to 32-bit protected mode by using a long jmp
    # to reload %cs and %eip.  The segment descriptors are set up with no
    # translation, so that the mapping is still the identity mapping.
    .att_syntax
    ljmp ${cs}, $start32
    .intel_syntax

    .code32  # Tell assembler to generate 32-bit code now.
    start32:
        # Set up the protected-mode data segment registers
        mov ax, {ds} # Our data segment selector
        mov ds, ax                # -> DS: Data Segment
        mov es, ax                # -> ES: Extra Segment
        mov ss, ax                # -> SS: Stack Segment
        mov ax, 0                 # Zero segments not ready for use
        mov fs, ax                # -> FS
        mov gs, ax                # -> GS

        lea esp, start
        call boot_main

        # If bootmain returns (it shouldn't), trigger a Bochs
        # breakpoint if running under Bochs, then loop.
        mov ax, 0x8a00           # 0x8a00 -> port 0x8a00
        mov dx, ax
        out dx, ax
        mov ax, 0x8ae0            # 0x8ae0 -> port 0x8a00
        out dx, ax
    
    spin:
        jmp spin
    "#,
    cr0 = const CR0_PROTECTION_ENABLE,
    cs = const (SEGMENT_KERNEL_CODE << 3),
    ds = const (SEGMENT_KERNEL_DATA << 3),
);

#[repr(C, align(4))]
struct Aligned4<T>(pub T);

#[repr(packed)]
struct Segment(u16, u16, u8, u8, u8, u8);

impl Segment {
    pub const TYPE_EXECUTABLE: u8 = 0x8;
    pub const TYPE_WRITABLE: u8 = 0x2;
    pub const TYPE_READABLE: u8 = 0x2;

    pub const fn null() -> Self {
        Self(0, 0, 0, 0, 0, 0)
    }

    pub const fn new(ty: u8, base: u32, limit: u32) -> Self {
        Self(
            ((limit >> 12) & 0xffff) as u16,
            (base & 0xffff) as u16,
            ((base >> 16) & 0xff) as u8,
            0x90 | ty,
            0xc0 | ((limit >> 28) & 0xf) as u8,
            ((base >> 24) & 0xff) as u8,
        )
    }
}

#[repr(packed)]
struct GdtDescriptor(pub u16, pub *const Segment);

// 本来、ポインタ型はSyncを満たさないためstatic変数に用いることはできない
// しかし、このブートコードはシングルスレッドで動作するため、安全であるとみなしてSyncを付ける
unsafe impl Sync for GdtDescriptor {}

#[repr(C)]
pub struct ElfHeader {
    pub magic: u32, // must equal ELF_MAGIC
    pub elf: [u8; 12],
    pub ty: u16,
    pub machine: u16,
    pub version: u32,
    pub entry: extern "C" fn(),
    pub phoff: u32,
    pub shoff: u32,
    pub flags: u32,
    pub ehsize: u16,
    pub phentsize: u16,
    pub phnum: u16,
    pub shentsize: u16,
    pub shnum: u16,
    pub shstrndx: u16,
}

impl ElfHeader {
    const MAGIC: u32 = 0x464C457F; // "\x7FELF" in little endian

    pub const fn is_elf(&self) -> bool {
        self.magic == Self::MAGIC
    }
}

// Program section header
#[repr(C)]
pub struct ProgramHeader {
    pub ty: u32,
    pub off: u32,
    pub vaddr: u32,
    pub paddr: u32,
    pub filesz: u32,
    pub memsz: u32,
    pub flags: u32,
    pub align: u32,
}

#[used]
#[no_mangle]
static GDT: Aligned4<[Segment; 3]> = Aligned4([
    Segment::null(),
    Segment::new(
        Segment::TYPE_EXECUTABLE | Segment::TYPE_READABLE,
        0x0,
        0xffffffff,
    ),
    Segment::new(Segment::TYPE_WRITABLE, 0x0, 0xffffffff),
]);

#[used]
#[no_mangle]
static GDT_DESC: GdtDescriptor = GdtDescriptor(core::mem::size_of_val(&GDT) as u16 - 1, unsafe {
    core::mem::transmute(core::ptr::addr_of!(GDT))
});

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

unsafe fn insl(port: u16, mut _addr: *mut u32, mut _count: usize) {
    asm!("cld; rep insl", in("dx") port, inout("ecx") _count, inout("edi") _addr, options(att_syntax));
}

unsafe fn inb(port: u16) -> u8 {
    let mut val;
    asm!("in al, dx", out("al") val, in("dx") port, options(nostack));
    val
}

unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nostack));
}

#[no_mangle]
unsafe extern "C" fn boot_main() {
    let elf = 0x10000 as *mut ElfHeader;

    // Read 1st page off disk
    read_segment(elf as *mut _, 4096, 0);

    // Is this an ELF executable?
    if !(*elf).is_elf() {
        return; // let bootasm.S handle error
    }

    // Load each program segment (ignores ph flags).
    let ph_offset = (*elf).phoff as usize;
    let ph_num = (*elf).phnum as usize;

    let mut ph = elf.cast::<u8>().add(ph_offset).cast::<ProgramHeader>();
    let end_ph = ph.add(ph_num);

    while ph < end_ph {
        let paddr = (*ph).paddr as *mut u8;
        let filesize = (*ph).filesz as usize;
        let memsize = (*ph).memsz as usize;
        let offset = (*ph).off as usize;

        read_segment(paddr, filesize, offset);

        if memsize > filesize {
            for i in 0..(memsize - filesize) {
                *paddr.add(filesize).add(i) = 0;
            }
            //core::ptr::write_bytes(paddr.add(filesize), 0, memsize - filesize);
        }

        ph = ph.add(1);
    }

    // Call the entry point from the ELF header.
    // Does not return!
    ((*elf).entry)();
}

#[inline(always)]
fn wait_disk() {
    // Wait for disk ready.
    while (unsafe { inb(0x1F7) } & 0xC0) != 0x40 {}
}

// Read a single sector at offset into dst.
#[inline(always)]
unsafe fn read_sector(dst: *mut u8, offset: usize) {
    // Issue command.
    wait_disk();
    outb(0x1F2, 1); // count = 1
    outb(0x1F3, offset as u8);
    outb(0x1F4, (offset >> 8) as u8);
    outb(0x1F5, (offset >> 16) as u8);
    outb(0x1F6, ((offset >> 24) | 0xE0) as u8);
    outb(0x1F7, 0x20); // cmd 0x20 - read sectors

    // Read data.
    wait_disk();
    insl(0x1F0, dst as *mut u32, SECTOR_SIZE / 4);
}

// Read 'count' bytes at 'offset' from kernel into physical address 'pa'.
// Might copy more than asked.
unsafe fn read_segment(paddr: *mut u8, count: usize, offset: usize) {
    let end_paddr = paddr.add(count);

    // Round down to sector boundary.
    let mut paddr = paddr.sub(offset % SECTOR_SIZE);

    // Translate from bytes to sectors; kernel starts at sector 1.
    let mut offset = (offset / SECTOR_SIZE) + 1;

    // If this is too slow, we could read lots of sectors at a time.
    // We'd write more to memory than asked, but it doesn't matter --
    // we load in increasing order.
    while paddr < end_paddr {
        read_sector(paddr, offset);

        paddr = paddr.add(SECTOR_SIZE);
        offset += 1;
    }
}
