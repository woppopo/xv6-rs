use core::arch::asm;
use seq_macro::seq;

pub type Trap = extern "C" fn() -> !;

seq!(N in 0..=255 {
    #[naked]
    extern "C" fn vector#N() -> ! {
        unsafe {
            asm! {
                r#"
                push 0
                push {}
                jmp alltraps
                "#, 
                const N,
                options(noreturn),
            }
        }
    }

    #[naked]
    extern "C" fn vector_with_error#N() -> ! {
        unsafe {
            asm! {
                r#"
                push {}
                jmp alltraps
                "#, 
                const N,
                options(noreturn),
            }
        }
    }
});

pub const fn trap_vector(index: usize) -> Trap {
    seq!(N in 0..=255 {
        const VECTORS: [Trap; 256] = [ #(vector#N,)* ];
        const VECTORS_WITH_ERROR: [Trap; 256] = [ #(vector_with_error#N,)* ];
    });

    if index == 8 || (index >= 10 && index <= 14) || index == 17 {
        VECTORS_WITH_ERROR[index]
    } else {
        VECTORS[index]
    }
}
