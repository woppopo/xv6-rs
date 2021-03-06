// Long-term locks for processes
struct sleeplock
{
  uint locked;        // Is the lock held?
  struct spinlock lk; // spinlock protecting this sleep lock
  int pid;            // Process holding lock
};
