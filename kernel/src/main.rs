#![no_std]
#![no_main]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[link(name = "main")]
extern "C" {
    fn main();
}

#[no_mangle]
extern "C" fn test() {
    unsafe {
        main();
    }
}
