use crate::trap::T_IRQ0;

pub struct IoApic {
    reg: *mut u32,
    data: *mut u32,
}

impl IoApic {
    pub const fn from_addr(addr: usize) -> Self {
        Self {
            reg: addr as *mut u32,
            data: (addr + 0x10) as *mut u32,
        }
    }

    pub fn read(&self, reg: u32) -> u32 {
        unsafe {
            self.reg.write_volatile(reg);
            self.data.read_volatile()
        }
    }

    pub fn write(&self, reg: u32, data: u32) {
        unsafe {
            self.reg.write_volatile(reg);
            self.data.write_volatile(data);
        }
    }
}

const IOAPIC_ADDR: usize = 0xFEC00000; // Default physical address of IO APIC
const IOAPIC_REG_ID: u32 = 0x00; // Register index: ID
const IOAPIC_REG_VER: u32 = 0x01; // Register index: version
const IOAPIC_REG_TABLE: u32 = 0x10; // Redirection table base

// The redirection table starts at REG_TABLE and uses
// two registers to configure each interrupt.
// The first (low) register in a pair contains configuration bits.
// The second (high) register contains a bitmask telling which
// CPUs can serve that interrupt.
const INT_DISABLED: u32 = 0x00010000; // Interrupt disabled

pub fn ioapicinit(ioapicid: u8) {
    let ioapic = IoApic::from_addr(IOAPIC_ADDR);

    let id = ioapic.read(IOAPIC_REG_ID);
    let ver = ioapic.read(IOAPIC_REG_VER);
    let max_interrupt = (ver >> 16) & 0xff;

    if id != ioapicid as u32 {
        unimplemented!()
        //cprintf("ioapicinit: id isn't equal to ioapicid; not a MP\n");
    }

    // Mark all interrupts edge-triggered, active high, disabled,
    // and not routed to any CPUs.
    for i in 0..=max_interrupt {
        ioapic.write(IOAPIC_REG_TABLE + 2 * i, INT_DISABLED | (T_IRQ0 + i));
        ioapic.write(IOAPIC_REG_TABLE + 2 * i + 1, 0);
    }
}

#[no_mangle]
extern "C" fn ioapicenable(irq: u32, cpunum: u32) {
    let ioapic = IoApic::from_addr(IOAPIC_ADDR);

    // Mark interrupt edge-triggered, active high,
    // enabled, and routed to the given cpunum,
    // which happens to be that cpu's APIC ID.
    ioapic.write(IOAPIC_REG_TABLE + 2 * irq, T_IRQ0 + irq);
    ioapic.write(IOAPIC_REG_TABLE + 2 * irq + 1, cpunum << 24);
}
