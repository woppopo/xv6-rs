pub unsafe fn inb(port: u16) -> u8 {
    let mut val;
    asm!("in al, dx", out("al") val, in("dx") port, options(nostack));
    val
}

pub unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nostack));
}

pub unsafe fn lcr3(val: u32) {
    asm!("mov cr3, {0}", in(reg) val, options(nostack));
}
