use core::arch::asm;

// Layout of the trap frame built on the stack by the
// hardware and by trapasm.S, and passed to trap().
#[repr(C)]
pub struct TrapFrame {
    // registers as pushed by pusha
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    pub oesp: u32, // useless & ignored
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,

    // rest of trap frame
    pub gs: u16,
    pub padding1: u16,
    pub fs: u16,
    pub padding2: u16,
    pub es: u16,
    pub padding3: u16,
    pub ds: u16,
    pub padding4: u16,
    pub trapno: u32,

    // below here defined by x86 hardware
    pub err: u32,
    pub eip: u32,
    pub cs: u16,
    pub padding5: u16,
    pub eflags: u32,

    // below here only when crossing rings, such as from user to kernel
    pub esp: u32,
    pub ss: u16,
    pub padding6: u16,
}

pub unsafe fn inb(port: u16) -> u8 {
    let mut val;
    asm!("in al, dx", out("al") val, in("dx") port, options(nostack));
    val
}

pub unsafe fn insl(port: u16, _addr: *mut u32, mut _count: usize) {
    asm!("cld; rep insl", in("dx") port, inout("ecx") _count, in("edi") _addr, options(att_syntax))
}

pub unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nostack));
}

pub unsafe fn outl(port: u16, val: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") val, options(nostack));
}

pub unsafe fn outsl(port: u16, addr: *const u32, count: usize) {
    //asm!("cld; rep outsl", in("dx") port, inout("ecx") count, in("esi") addr, options(att_syntax));
    for i in 0..count {
        let ptr = addr.add(i);
        outl(port, *ptr);
    }
}

pub unsafe fn stosb(addr: *mut u8, val: u8, count: usize) {
    //asm!("cld; rep stosl", in("edi") addr, in("eax") val, in("ecx") count, options(att_syntax));
    for i in 0..count {
        let ptr = addr.add(i);
        *ptr = val;
    }
}

pub unsafe fn stosl(addr: *mut u32, val: u32, count: usize) {
    //asm!("cld; rep stosl", in("edi") addr, in("eax") val, in("ecx") count, options(att_syntax));
    for i in 0..count {
        let ptr = addr.add(i);
        *ptr = val;
    }
}

pub unsafe fn lcr3(val: usize) {
    asm!("mov cr3, {0}", in(reg) val, options(nostack));
}

pub unsafe fn ltr(selector: u16) {
    asm!("ltr ax", in("ax") selector, options(nostack));
}

pub unsafe fn readeflags() -> u32 {
    let mut eflags;
    asm!("pushfd; pop {0}", out(reg) eflags);
    eflags
}

pub unsafe fn cli() {
    asm!("cli");
}

pub unsafe fn sti() {
    asm!("sti");
}

pub unsafe fn lidt<T, const SIZE: usize>(table: &[T; SIZE]) {
    let size_of_bytes = core::mem::size_of::<T>() * SIZE;
    let pd = [
        (size_of_bytes - 1) as u16,
        table as *const T as usize as u16,
        (table as *const T as usize >> 16) as u16,
    ];

    asm!("lidt [{0}]", in(reg) pd.as_ptr());
}
