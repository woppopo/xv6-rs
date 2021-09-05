struct LocalApic {
    head: *mut u32,
}

impl LocalApic {
    pub const REG_ID: usize = 0x0020; // ID
    pub const REG_VER: usize = 0x0030; // Version
    pub const REG_TPR: usize = 0x0080; // Task Priority
    pub const REG_EOI: usize = 0x00B0; // EOI
    pub const REG_SVR: usize = 0x00F0; // Spurious Interrupt Vector
    pub const REG_ESR: usize = 0x0280; // Error Status
    pub const REG_ICRLO: usize = 0x0300; // Interrupt Command
    pub const REG_ICRHI: usize = 0x0310; // Interrupt Command [63:32]
    pub const REG_TIMER: usize = 0x0320; // Local Vector Table 0 (TIMER)
    pub const REG_PCINT: usize = 0x0340; // Performance Counter LVT
    pub const REG_LINT0: usize = 0x0350; // Local Vector Table 1 (LINT0)
    pub const REG_LINT1: usize = 0x0360; // Local Vector Table 2 (LINT1)
    pub const REG_ERROR: usize = 0x0370; // Local Vector Table 3 (ERROR)
    pub const REG_TICR: usize = 0x0380; // Timer Initial Count
    pub const REG_TDCR: usize = 0x03E0; // Timer Divide Configuration

    pub const SVR_ENABLE: u32 = 0x00000100; // Unit Enable
    pub const ICRLO_INIT: u32 = 0x00000500; // INIT/RESET
    pub const ICRLO_STARTUP: u32 = 0x00000600; // Startup IPI
    pub const ICRLO_DELIVS: u32 = 0x00001000; // Delivery status
    pub const ICRLO_ASSERT: u32 = 0x00004000; // Assert interrupt (vs deassert)
    pub const ICRLO_LEVEL: u32 = 0x00008000; // Level triggered
    pub const ICRLO_BCAST: u32 = 0x00080000; // Send to all APICs, including self.
    pub const TIMER_SPERIODIC: u32 = 0x00020000; // Periodic
    pub const INT_MASKED: u32 = 0x00010000; // Interrupt masked
    pub const TDCR_X1: u32 = 0x0000000B; // divide counts by 1

    pub const fn new(addr: *mut u32) -> Self {
        Self { head: addr }
    }

    pub fn write(&self, index: usize, value: u32) {
        unsafe {
            self.head.add(index / 4).write_volatile(value);
        }
    }

    pub fn read(&self, index: usize) -> u32 {
        unsafe { self.head.add(index / 4).read_volatile() }
    }
}

static mut LAPIC: Option<LocalApic> = None;

#[no_mangle]
pub extern "C" fn lapicinit(addr: *mut u32) {
    use crate::trap::{IRQ_ERROR, IRQ_SPURIOUS, IRQ_TIMER, T_IRQ0};

    let lapic = unsafe {
        LAPIC = Some(LocalApic::new(addr));
        LAPIC.as_mut().unwrap()
    };

    // Enable local APIC; set spurious interrupt vector.
    lapic.write(
        LocalApic::REG_SVR,
        LocalApic::SVR_ENABLE | (T_IRQ0 + IRQ_SPURIOUS),
    );

    // The timer repeatedly counts down at bus frequency
    // from lapic[TICR] and then issues an interrupt.
    // If xv6 cared more about precise timekeeping,
    // TICR would be calibrated using an external time source.
    lapic.write(LocalApic::REG_TDCR, LocalApic::TDCR_X1);
    lapic.write(
        LocalApic::REG_TIMER,
        LocalApic::TIMER_SPERIODIC | (T_IRQ0 + IRQ_TIMER),
    );
    lapic.write(LocalApic::REG_TICR, 10000000);

    // Disable logical interrupt lines.
    lapic.write(LocalApic::REG_LINT0, LocalApic::INT_MASKED);
    lapic.write(LocalApic::REG_LINT1, LocalApic::INT_MASKED);

    // Disable performance counter overflow interrupts
    // on machines that provide that interrupt entry.
    if ((lapic.read(LocalApic::REG_VER) >> 16) & 0xFF) >= 4 {
        lapic.write(LocalApic::REG_PCINT, LocalApic::INT_MASKED);
    }

    // Map error interrupt to IRQ_ERROR.
    lapic.write(LocalApic::REG_ERROR, T_IRQ0 + IRQ_ERROR);

    // Clear error status register (requires back-to-back writes).
    lapic.write(LocalApic::REG_ESR, 0);
    lapic.write(LocalApic::REG_ESR, 0);

    // Ack any outstanding interrupts.
    lapic.write(LocalApic::REG_EOI, 0);

    // Send an Init Level De-Assert to synchronise arbitration ID's.
    lapic.write(LocalApic::REG_ICRHI, 0);
    lapic.write(
        LocalApic::REG_ICRLO,
        LocalApic::ICRLO_BCAST | LocalApic::ICRLO_INIT | LocalApic::ICRLO_LEVEL,
    );
    while lapic.read(LocalApic::REG_ICRLO) & LocalApic::ICRLO_DELIVS != 0 {}

    // Enable interrupts on the APIC (but not on the processor).
    lapic.write(LocalApic::REG_TPR, 0);
}

#[no_mangle]
extern "C" fn lapicid() -> u32 {
    unsafe {
        LAPIC
            .as_ref()
            .map(|lapic| lapic.read(LocalApic::REG_ID) >> 24)
            .unwrap_or(0)
    }
}

#[no_mangle]
extern "C" fn lapiceoi() {
    unsafe {
        LAPIC
            .as_ref()
            .map(|lapic| lapic.write(LocalApic::REG_EOI, 0));
    }
}

pub fn microdelay(_ms: u32) {}

// Start additional processor running entry code at addr.
// See Appendix B of MultiProcessor Specification.
#[no_mangle]
extern "C" fn lapicstartap(apicid: u8, addr: u32) {
    use crate::memlayout::p2v;
    use crate::x86::outb;

    const CMOS_PORT: u16 = 0x70;
    const CMOS_RETURN: u16 = 0x71;

    // "The BSP must initialize CMOS shutdown code to 0AH
    // and the warm reset vector (DWORD based at 40:67) to point at
    // the AP startup code prior to the [universal startup algorithm]."
    unsafe {
        outb(CMOS_PORT, 0xF); // offset 0xF is shutdown code
        outb(CMOS_PORT + 1, 0x0A);

        let wrv = p2v(0x40 << 4 | 0x67) as *mut u16; // Warm reset vector
        *wrv.add(0) = 0;
        *wrv.add(1) = (addr >> 4) as u16;
    }

    let lapic = unsafe { LAPIC.as_ref().unwrap() };

    // "Universal startup algorithm."
    // Send INIT (level-triggered) interrupt to reset other CPU.
    lapic.write(LocalApic::REG_ICRHI, (apicid as u32) << 24);
    lapic.write(
        LocalApic::REG_ICRLO,
        LocalApic::ICRLO_INIT | LocalApic::ICRLO_LEVEL | LocalApic::ICRLO_ASSERT,
    );
    microdelay(200);
    lapic.write(
        LocalApic::REG_ICRLO,
        LocalApic::ICRLO_INIT | LocalApic::ICRLO_LEVEL,
    );
    microdelay(100); // should be 10ms, but too slow in Bochs!

    // Send startup IPI (twice!) to enter code.
    // Regular hardware is supposed to only accept a STARTUP
    // when it is in the halted state due to an INIT.  So the second
    // should be ignored, but it is part of the official Intel algorithm.
    // Bochs complains about the second one.  Too bad for Bochs.
    for _ in 0..2 {
        lapic.write(LocalApic::REG_ICRHI, (apicid as u32) << 24);
        lapic.write(
            LocalApic::REG_ICRLO,
            LocalApic::ICRLO_STARTUP | (addr >> 12),
        );
        microdelay(200);
    }
}
