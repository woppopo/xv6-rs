use crate::buf::Buffer;

extern "C" {
    pub fn iderw(b: *mut Buffer);
}
