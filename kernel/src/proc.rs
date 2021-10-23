use core::{ffi::c_void, sync::atomic::AtomicU32};

use crate::{
    file::{File, INode},
    mmu::{SegmentDescriptorTable, TaskState},
    param::NOFILE,
    spinlock::SpinLock,
    vm::PDE,
    x86::TrapFrame,
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
    sz: u32,                  // Size of process memory (bytes)
    pub pgdir: *const PDE,    // Page table
    pub kstack: *const u8,    // Bottom of kernel stack for this process
    pub state: ProcessState,  // Process state
    pub pid: i32,             // Process ID
    parent: *const Self,      // Parent process
    pub tf: *const TrapFrame, // Trap frame for current syscall
    context: *const Context,  // swtch() here to run process
    chan: *const c_void,      // If non-zero, sleeping on chan
    pub killed: i32,          // If non-zero, have been killed
    ofile: [File; NOFILE],    // Open files
    cwd: *const INode,        // Current directory
    name: [i8; 16],           // Process name (debugging)
}

extern "C" {
    pub fn mycpu() -> *mut Cpu;
    pub fn myproc() -> *mut Process;
    pub fn wakeup(chan: *const c_void);
    pub fn sleep(chan: *const c_void, lk: *const SpinLock);
    pub fn exit();
    pub fn yield_proc();
    pub fn cpuid() -> i32;
}
