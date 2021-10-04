use crate::{
    console::consoleintr,
    x86::{inb, outb},
};

const COM1: u16 = 0x3f8;

static mut UART_INITIALIZED: bool = false;

pub fn uartinit() {
    use crate::ioapic::ioapicenable;
    use crate::trap::IRQ_COM1;

    unsafe {
        // Turn off the FIFO
        outb(COM1 + 2, 0);

        // 9600 baud, 8 data bits, 1 stop bit, parity off.
        outb(COM1 + 3, 0x80); // Unlock divisor
        outb(COM1 + 0, (115200 / 9600) as u8);
        outb(COM1 + 1, 0);
        outb(COM1 + 3, 0x03); // Lock divisor, 8 data bits.
        outb(COM1 + 4, 0);
        outb(COM1 + 1, 0x01); // Enable receive interrupts.

        // If status is 0xFF, no serial port.
        if inb(COM1 + 5) == 0xFF {
            return;
        }

        UART_INITIALIZED = true;

        // Acknowledge pre-existing interrupt conditions;
        // enable interrupts.
        inb(COM1 + 2);
        inb(COM1 + 0);
        ioapicenable(IRQ_COM1, 0);

        // Announce that we're here.
        for ch in "xv6..\n".chars() {
            uartputc(ch as u32);
        }
    }
}

#[no_mangle]
pub extern "C" fn uartputc(c: u32) {
    use crate::lapic::microdelay;

    unsafe {
        if !UART_INITIALIZED {
            return;
        }

        for _ in (0..128).take_while(|_| inb(COM1 + 5) & 0x20 == 0) {
            microdelay(10);
        }

        outb(COM1 + 0, c as u8);
    }
}

extern "C" fn uartgetc() -> u32 {
    unsafe {
        if !UART_INITIALIZED {
            return u32::MAX;
        }

        if inb(COM1 + 5) & 0x01 == 0 {
            return u32::MAX;
        }

        inb(COM1 + 0) as u32
    }
}

#[no_mangle]
pub extern "C" fn uartintr() {
    unsafe {
        consoleintr(uartgetc);
    }
}
