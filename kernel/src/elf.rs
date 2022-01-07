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
