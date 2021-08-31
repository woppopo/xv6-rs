#![no_std]
#![no_main]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[repr(transparent)]
struct StaticPointer<T>(*const T);

unsafe impl<T> Sync for StaticPointer<T> {}

#[used]
#[no_mangle]
static INITCODE: &'static [u8] = include_bytes!(concat!(env!("OUT_DIR"), "/initcode"));

#[used]
#[no_mangle]
static _binary_initcode_start: StaticPointer<u8> = StaticPointer(INITCODE.as_ptr());

#[used]
#[no_mangle]
static _binary_initcode_size: usize = INITCODE.len();

#[used]
#[no_mangle]
static ENTRYOTHER: &'static [u8] = include_bytes!(concat!(env!("OUT_DIR"), "/entryother"));

#[used]
#[no_mangle]
static _binary_entryother_start: StaticPointer<u8> = StaticPointer(ENTRYOTHER.as_ptr());

#[used]
#[no_mangle]
static _binary_entryother_size: usize = ENTRYOTHER.len();
