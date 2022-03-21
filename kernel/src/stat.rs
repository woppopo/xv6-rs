pub enum FileType {
    Directory,
    File,
    Device,
}

#[repr(C)]
pub struct Stat {
    kind: i16,  // Type of file
    dev: i32,   // File system's disk device
    ino: u32,   // Inode number
    nlink: i16, // Number of links to file
    size: u32,  // Size of file in bytes
}
