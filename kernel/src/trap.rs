use crate::{
    ide::ide_interrupt,
    keyboard::keyboard_interrupt,
    lapic::lapiceoi,
    mmu::SegmentDescriptorTable,
    proc::{exit, my_cpu_id, myproc, wakeup, yield_proc, ProcessState},
    spinlock::SpinLockC,
    syscall::syscall,
    trapvec::trap_vector,
    x86::{lidt, TrapFrame},
};

// x86 trap and interrupt constants.

// Processor-defined:
pub const T_DIVIDE: u32 = 0; // divide error
pub const T_DEBUG: u32 = 1; // debug exception
pub const T_NMI: u32 = 2; // non-maskable interrupt
pub const T_BRKPT: u32 = 3; // breakpoint
pub const T_OFLOW: u32 = 4; // overflow
pub const T_BOUND: u32 = 5; // bounds check
pub const T_ILLOP: u32 = 6; // illegal opcode
pub const T_DEVICE: u32 = 7; // device not available
pub const T_DBLFLT: u32 = 8; // double fault
pub const T_COPROC: u32 = 9; // reserved (not used since 486)
pub const T_TSS: u32 = 10; // invalid task switch segment
pub const T_SEGNP: u32 = 11; // segment not present
pub const T_STACK: u32 = 12; // stack exception
pub const T_GPFLT: u32 = 13; // general protection fault
pub const T_PGFLT: u32 = 14; // page fault
pub const T_RES: u32 = 15; // reserved
pub const T_FPERR: u32 = 16; // floating point error
pub const T_ALIGN: u32 = 17; // aligment check
pub const T_MCHK: u32 = 18; // machine check
pub const T_SIMDERR: u32 = 19; // SIMD floating point error

// These are arbitrarily chosen, but with care not to overlap
// processor defined exceptions or interrupt vectors.
pub const T_SYSCALL: u32 = 64; // system call
pub const T_DEFAULT: u32 = 500; // catchall

pub const T_IRQ0: u32 = 32; // IRQ 0 corresponds to int T_IRQ

pub const IRQ_TIMER: u32 = 0;
pub const IRQ_KBD: u32 = 1;
pub const IRQ_COM1: u32 = 4;
pub const IRQ_IDE: u32 = 14;
pub const IRQ_ERROR: u32 = 19;
pub const IRQ_SPURIOUS: u32 = 31;

// Gate descriptors for interrupts and traps
/*
off_15_0 : 16;   // low 16 bits of offset in segment
cs : 16;         // code segment selector
args : 5;        // # args, 0 for interrupt/trap gates
rsv1 : 3;        // reserved(should be zero I guess)
type : 4;        // type(STS_{IG32,TG32})
s : 1;           // must be 0 (system)
dpl : 2;         // descriptor(meaning new) privilege level
p : 1;           // Present
off_31_16 : 16;  // high bits of offset in segment
*/
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct GateDescriptor(u32, u32);

impl GateDescriptor {
    pub const TYPE_INTERRUPT_GATE_32: u8 = 0xe;
    pub const TYPE_TRAP_GATE_32: u8 = 0xf;

    pub const fn null() -> Self {
        Self(0, 0)
    }

    // Set up a normal interrupt/trap gate descriptor.
    // - is_trap: 1 for a trap (= exception) gate, 0 for an interrupt gate.
    //   interrupt gate clears FL_IF, trap gate leaves FL_IF alone
    // - cs: Code segment selector for interrupt/trap handler
    // - offset: Offset in code segment for interrupt/trap handler
    // - dpl: Descriptor Privilege Level -
    //        the privilege level required for software to invoke
    //        this interrupt/trap gate explicitly using an int instruction.
    pub const fn new(is_trap: bool, cs: u16, offset: u32, dpl: u8) -> Self {
        let desc0 = (cs as u32) << 16 | (offset & 0xffff);
        let desc1 = (offset >> 16) << 16
            | 1 << 15
            | (dpl as u32) << 13
            | 0 << 12
            | if is_trap {
                Self::TYPE_TRAP_GATE_32 as u32
            } else {
                Self::TYPE_INTERRUPT_GATE_32 as u32
            } << 8
            | 0 << 5
            | 0;

        Self(desc0, desc1)
    }
}

fn init_trap_vector_table(idt: &mut [GateDescriptor; 256]) {
    for i in 0..256 {
        idt[i] = GateDescriptor::new(
            false,
            SegmentDescriptorTable::KERNEL_CODE_SELECTOR,
            trap_vector(i) as u32,
            0,
        );
    }

    let syscall_at = T_SYSCALL as usize;
    idt[syscall_at] = GateDescriptor::new(
        true,
        SegmentDescriptorTable::KERNEL_CODE_SELECTOR,
        trap_vector(syscall_at) as u32,
        3,
    );
}

static mut IDT: [GateDescriptor; 256] = [GateDescriptor::null(); 256];

#[no_mangle]
static mut TICKS: u32 = 0;

#[no_mangle]
static mut TICKSLOCK: SpinLockC = SpinLockC::new();

pub fn load_interrupt_descriptor_table() {
    unsafe {
        init_trap_vector_table(&mut IDT); // TODO: prevent multiple initialization
        lidt(&IDT);
    }
}

unsafe fn trap_handler(tf: &mut TrapFrame) {
    use crate::uart::uart_interrupt_handler;

    if tf.trapno == T_SYSCALL {
        if (*myproc()).killed != 0 {
            exit()
        }

        (*myproc()).tf = tf;
        syscall();

        if (*myproc()).killed != 0 {
            exit()
        }

        return;
    }

    match tf.trapno {
        const { T_IRQ0 + IRQ_TIMER } => {
            if my_cpu_id() == 0 {
                TICKSLOCK.acquire();
                TICKS += 1;
                wakeup(&TICKS as *const _ as *const _);
                TICKSLOCK.release();
            }
            lapiceoi();
        }
        const { T_IRQ0 + IRQ_IDE } => {
            ide_interrupt();
            lapiceoi();
        }
        const { T_IRQ0 + IRQ_IDE + 1 } => {
            // Bochs generates spurious IDE1 interrupts.
        }
        const { T_IRQ0 + IRQ_KBD } => {
            keyboard_interrupt();
            lapiceoi();
        }
        const { T_IRQ0 + IRQ_COM1 } => {
            uart_interrupt_handler();
            lapiceoi();
        }
        const { T_IRQ0 + 7 } | const { T_IRQ0 + IRQ_SPURIOUS } => {
            //cprintf("cpu%d: spurious interrupt at %x:%x\n", cpuid(), tf->cs, tf->eip);
            lapiceoi();
        }
        _ => {
            if myproc().is_null() || tf.cs & 3 == 0 {
                // In kernel, it must be our mistake.
                //cprintf("unexpected trap %d from cpu %d eip %x (cr2=0x%x)\n", tf->trapno, cpuid(), tf->eip, rcr2());
                panic!("trap");
            }
            // In user space, assume process misbehaved.
            //cprintf("pid %d %s: trap %d err %d on cpu %d eip 0x%x addr 0x%x--kill proc\n", myproc()->pid, myproc()->name, tf->trapno, tf->err, cpuid(), tf->eip, rcr2());
            (*myproc()).killed = 1;
        }
    }

    // Force process exit if it has been killed and is in user space.
    // (If it is still executing in the kernel, let it keep running
    // until it gets to the regular system call return.)
    if !myproc().is_null() && (*myproc()).killed != 0 && (tf.cs & 3) == 3 {
        exit();
    }

    // Force process to give up CPU on clock tick.
    // If interrupts were on while locks held, would need to check nlock.
    if !myproc().is_null()
        && (*myproc()).state == ProcessState::Running
        && tf.trapno == T_IRQ0 + IRQ_TIMER
    {
        yield_proc();
    }

    // Check if the process has been killed since we yielded
    if !myproc().is_null() && (*myproc()).killed != 0 && (tf.cs & 3) == 3 {
        exit();
    }
}

mod _bindings {
    use super::*;

    #[no_mangle]
    extern "C" fn trap(tf: *mut TrapFrame) {
        unsafe {
            trap_handler(&mut *tf);
        }
    }
}
