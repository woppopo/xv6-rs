extern "C" {
    pub fn consoleintr(handler: extern "C" fn() -> u32);
}
