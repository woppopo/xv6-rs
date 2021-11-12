use core::{ffi::c_void, sync::atomic::AtomicU32};

use arrayvec::ArrayVec;

use crate::{
    file::{File, INode},
    lapic::lapicid,
    mmu::{SegmentDescriptorTable, TaskState, FL_IF},
    param::{NOFILE, NPROC},
    spinlock::SpinLockC,
    vm::PDE,
    x86::{readeflags, TrapFrame},
    CPUS,
};

// Per-CPU state
#[repr(C)]
pub struct Cpu {
    pub apicid: u8,                  // Local APIC ID
    scheduler: *const Context,       // swtch() here to enter scheduler
    pub ts: TaskState,               // Used by x86 to find stack for interrupt
    pub gdt: SegmentDescriptorTable, // x86 global descriptor table
    pub started: AtomicU32,          // Has the CPU started?
    pub ncli: i32,                   // Depth of pushcli nesting.
    pub intena: u32,                 // Were interrupts enabled before pushcli?
    proc: *const Process,            // The process running on this cpu or null
}

// Saved registers for kernel context switches.
// Don't need to save all the segment registers (%cs, etc),
// because they are constant across kernel contexts.
// Don't need to save %eax, %ecx, %edx, because the
// x86 convention is that the caller has saved them.
// Contexts are stored at the bottom of the stack they
// describe; the stack pointer is the address of the context.
// The layout of the context matches the layout of the stack in swtch.S
// at the "Switch stacks" comment. Switch doesn't save eip explicitly,
// but it is on the stack and allocproc() manipulates it.
#[repr(C)]
pub struct Context {
    edi: u32,
    esi: u32,
    ebx: u32,
    ebp: u32,
    eip: u32,
}

#[repr(C)]
#[derive(PartialEq)]
pub enum ProcessState {
    Unused,
    Embryo,
    Sleeping,
    Runnable,
    Running,
    Zombie,
}

// Per-process state
#[repr(C)]
pub struct Process {
    pub sz: usize,           // Size of process memory (bytes)
    pub pgdir: *const PDE,   // Page table
    pub kstack: *const u8,   // Bottom of kernel stack for this process
    pub state: ProcessState, // Process state
    pub pid: i32,            // Process ID
    parent: *const Self,     // Parent process
    pub tf: *mut TrapFrame,  // Trap frame for current syscall
    context: *const Context, // swtch() here to run process
    chan: *const c_void,     // If non-zero, sleeping on chan
    pub killed: i32,         // If non-zero, have been killed
    ofile: [File; NOFILE],   // Open files
    cwd: *const INode,       // Current directory
    name: [i8; 16],          // Process name (debugging)
}

struct ProcessTable {
    lock: SpinLockC,
    init: *mut Process,
    procs: ArrayVec<Process, NPROC>,
    nextpid: u32,
}

impl ProcessTable {
    pub const fn new() -> Self {
        Self {
            lock: SpinLockC::new(),
            init: core::ptr::null_mut(),
            procs: ArrayVec::new_const(),
            nextpid: 1,
        }
    }
}

static mut PROCS: ProcessTable = ProcessTable::new();

// Must be called with interrupts disabled
pub fn my_cpu_id() -> usize {
    if unsafe { readeflags() & FL_IF != 0 } {
        panic!("cpuid called with interrupts enabled");
    }

    let apicid = lapicid() as u8;
    unsafe {
        CPUS.assume_init_ref()
            .iter()
            .position(|cpu| cpu.apicid == apicid)
            .expect("unknown apicid")
    }
}

// Must be called with interrupts disabled to avoid the caller being
// rescheduled between reading lapicid and running through the loop.
pub fn my_cpu() -> &'static Cpu {
    let id = my_cpu_id();
    let cpus = unsafe { CPUS.assume_init_ref() };
    &cpus[id]
}

// Must be called with interrupts disabled to avoid the caller being
// rescheduled between reading lapicid and running through the loop.
pub fn my_cpu_mut() -> &'static mut Cpu {
    let id = my_cpu_id();
    let cpus = unsafe { CPUS.assume_init_mut() };
    &mut cpus[id]
}

extern "C" {
    pub fn myproc() -> *mut Process;
    pub fn wakeup(chan: *const c_void);
    pub fn sleep(chan: *const c_void, lk: *const SpinLockC);
    pub fn exit();
    pub fn yield_proc();
}

mod _bindings {
    use super::*;

    #[no_mangle]
    extern "C" fn mycpu() -> *mut Cpu {
        my_cpu_mut()
    }
}
