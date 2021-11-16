use crate::{
    mmu::FL_IF,
    proc::my_cpu_mut,
    x86::{cli, readeflags, sti},
};

pub fn free<R>(f: impl FnOnce() -> R) -> R {
    push_cli();
    let ret = f();
    pop_cli();
    ret
}

// Pushcli/popcli are like cli/sti except that they are matched:
// it takes two popcli to undo two pushcli.  Also, if interrupts
// are off, then pushcli, popcli leaves them off.
pub fn push_cli() {
    let eflags = unsafe { readeflags() };
    unsafe {
        cli();
    }

    let cpu = my_cpu_mut();
    if cpu.ncli == 0 {
        cpu.intena = eflags & FL_IF;
    }
    cpu.ncli += 1;
}

pub fn pop_cli() {
    if unsafe { readeflags() & FL_IF != 0 } {
        panic!("pop_cli - interruptible");
    }

    let cpu = my_cpu_mut();
    if cpu.ncli == 0 {
        panic!("pop_cli");
    }

    cpu.ncli -= 1;

    if cpu.ncli == 0 && cpu.intena != 0 {
        unsafe {
            sti();
        }
    }
}

mod _binding {
    use super::*;

    #[no_mangle]
    extern "C" fn pushcli() {
        push_cli();
    }

    #[no_mangle]
    extern "C" fn popcli() {
        pop_cli();
    }
}
