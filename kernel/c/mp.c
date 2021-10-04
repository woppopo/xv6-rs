// Multiprocessor support
// Search memory for MP description structures.
// http://developer.intel.com/design/pentium/datashts/24201606.pdf

#include "types.h"
#include "defs.h"
#include "param.h"
#include "memlayout.h"
#include "mp.h"
#include "x86.h"
#include "mmu.h"
#include "proc.h"

struct cpu cpus[NCPU];
int ncpu;
uchar ioapicid;
volatile uint *lapic;

extern uchar sum(uchar *addr, int len);
extern struct mp *mpsearch();
extern struct mpconf *mpconfig(struct mp **pmp);

void mpinit(void)
{
  uchar *p, *e;
  int ismp;
  struct mp *mp;
  struct mpconf *conf;
  struct mpproc *proc;
  struct mpioapic *ioapic;

  if ((conf = mpconfig(&mp)) == 0)
    panic("Expect to run on an SMP");
  ismp = 1;
  lapic = (uint *)conf->lapicaddr;
  for (p = (uchar *)(conf + 1), e = (uchar *)conf + conf->length; p < e;)
  {
    switch (*p)
    {
    case MPPROC:
      proc = (struct mpproc *)p;
      if (ncpu < NCPU)
      {
        cpus[ncpu].apicid = proc->apicid; // apicid may differ from ncpu
        ncpu++;
      }
      p += sizeof(struct mpproc);
      continue;
    case MPIOAPIC:
      ioapic = (struct mpioapic *)p;
      ioapicid = ioapic->apicno;
      p += sizeof(struct mpioapic);
      continue;
    case MPBUS:
    case MPIOINTR:
    case MPLINTR:
      p += 8;
      continue;
    default:
      ismp = 0;
      break;
    }
  }
  if (!ismp)
    panic("Didn't find a suitable machine");

  if (mp->imcrp)
  {
    // Bochs doesn't support IMCR, so this doesn't run on Bochs.
    // But it would on real hardware.
    outb(0x22, 0x70);          // Select IMCR
    outb(0x23, inb(0x23) | 1); // Mask external interrupts.
  }
}
