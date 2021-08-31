const SEG_KDATA: u32 = 2;

#[repr(C)]
pub struct TrapFrame {
    // registers as pushed by pusha
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    pub oesp: u32, // useless & ignored
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,

    // rest of trap frame
    pub gs: u32,
    pub fs: u32,
    pub es: u16,
    pub ds: u16,

    pub trapno: u32,

    // below here defined by x86 hardware
    pub err: u32,
    pub eip: u32,
    pub cs: u16,
    pub padding5: u16,
    pub eflags: u32,

    // below here only when crossing rings, such as from user to kernel
    pub esp: u32,
    pub ss: u16,
    pub padding6: u16,
}

impl TrapFrame {
    pub fn zero() -> Self {
        Self {
            edi: 0,
            esi: 0,
            ebp: 0,
            oesp: 0,
            ebx: 0,
            edx: 0,
            ecx: 0,
            eax: 0,
            gs: 0,
            fs: 0,
            es: 0,
            ds: 0,
            trapno: 0,
            err: 0,
            eip: 0,
            cs: 0,
            padding5: 0,
            eflags: 0,
            esp: 0,
            ss: 0,
            padding6: 0,
        }
    }
}

global_asm!(
    r#"
    # vectors.S sends all traps here.
    .globl alltraps
    alltraps:
        # Build trap frame.
        pushl %ds
        pushl %es
        pushl %fs
        pushl %gs
        pushal
        
        # Set up data segments.
        movw ${ds}, %ax
        movw %ax, %ds
        movw %ax, %es
        
        # Call trap(tf), where tf=%esp
        pushl %esp
        call trap
        addl $4, %esp
    
    # Return falls through to trapret...
    .globl trapret
    trapret:
        popal
        popl %gs
        popl %fs
        popl %es
        popl %ds
        addl $0x8, %esp  # trapno and errcode
        iret
    "#,
    ds = const (SEG_KDATA << 3),
    options(att_syntax)
);

extern "C" {
    pub fn trapret();
}
