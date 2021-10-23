use seq_macro::seq;

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

#[no_mangle]
pub extern "C" fn trap_vector(index: usize) -> usize {
    seq!(N in 0..=255 {
        const VECTORS: [extern "C" fn() -> !; 256] = [ #(vector#N,)* ];
        const VECTORS_WITH_ERROR: [extern "C" fn() -> !; 256] = [ #(vector_with_error#N,)* ];
    });

    if index == 8 || (index >= 10 && index <= 14) || index == 17 {
        VECTORS_WITH_ERROR[index] as usize
    } else {
        VECTORS[index] as usize
    }
}
