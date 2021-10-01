pub const PGSIZE: usize = 4096;

pub const fn pg_roundup(size: usize) -> usize {
    (size + PGSIZE - 1) & !(PGSIZE - 1)
}

pub const fn pg_rounddown(size: usize) -> usize {
    size & !(PGSIZE - 1)
}
