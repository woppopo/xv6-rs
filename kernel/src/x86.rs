// Layout of the trap frame built on the stack by the
// hardware and by trapasm.S, and passed to trap().
#[repr(C)]
pub struct TrapFrame {
    // registers as pushed by pusha
    edi: u32,
    esi: u32,
    ebp: u32,
    oesp: u32, // useless & ignored
    ebx: u32,
    edx: u32,
    ecx: u32,
    eax: u32,

    // rest of trap frame
    gs: u16,
    padding1: u16,
    fs: u16,
    padding2: u16,
    es: u16,
    padding3: u16,
    ds: u16,
    padding4: u16,
    trapno: u32,

    // below here defined by x86 hardware
    err: u32,
    eip: u32,
    cs: u16,
    padding5: u16,
    eflags: u32,

    // below here only when crossing rings, such as from user to kernel
    esp: u32,
    ss: u16,
    padding6: u16,
}

pub unsafe fn inb(port: u16) -> u8 {
    let mut val;
    asm!("in al, dx", out("al") val, in("dx") port, options(nostack));
    val
}

pub unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nostack));
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
