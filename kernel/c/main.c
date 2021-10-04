#include "types.h"
#include "defs.h"
#include "param.h"
#include "memlayout.h"
#include "mmu.h"
#include "proc.h"
#include "x86.h"

// Common CPU setup code.
void mpmain(void)
{
  cprintf("cpu%d: starting %d\n", cpuid(), cpuid());
  idtinit();                    // load idt register
  xchg(&(mycpu()->started), 1); // tell startothers() we're up
  scheduler();                  // start running processes
}

// Other CPUs jump here from entryother.S.
void mpenter(void)
{
  switchkvm();
  seginit();
  lapicinit(LAPIC);
  mpmain();
}

extern pde_t ENTRYPGDIR[]; // For entry.S

// Start the non-boot (AP) processors.
void startothers(void)
{
  extern uchar *_binary_entryother_start;
  extern uint _binary_entryother_size;
  uchar *code;
  struct cpu *c;
  char *stack;

  // Write entry code to unused memory at 0x7000.
  // The linker has placed the image of entryother.S in
  // _binary_entryother_start.
  code = P2V(0x7000);
  memmove(code, _binary_entryother_start, (uint)_binary_entryother_size);

  for (c = CPUS; c < CPUS + NCPU; c++)
  {
    if (c == mycpu()) // We've started already.
      continue;

    // Tell entryother.S what stack to use, where to enter, and what
    // pgdir to use. We cannot use kpgdir yet, because the AP processor
    // is running in low  memory, so we use ENTRYPGDIR for the APs too.
    stack = kalloc();
    *(void **)(code - 4) = stack + KSTACKSIZE;
    *(void (**)(void))(code - 8) = mpenter;
    *(int **)(code - 12) = (void *)V2P(ENTRYPGDIR);

    lapicstartap(c->apicid, V2P(code));

    // wait for cpu to finish mpmain()
    while (c->started == 0)
      ;
  }
}
