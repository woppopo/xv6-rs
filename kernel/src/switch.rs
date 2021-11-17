use crate::proc::Context;

global_asm!(
    r#"
    .globl swtch
    swtch:
        # from
        mov eax, [esp+4]
        # to
        mov edx, [esp+8]
        # Save old callee-saved registers
        push ebp
        push ebx
        push esi
        push edi
        # Switch stacks
        mov [eax], esp
        mov esp, edx
        # Load new callee-saved registers
        pop edi
        pop esi
        pop ebx
        pop ebp
        ret
    "#
);

extern "C" {
    pub fn swtch(from: *mut *mut Context, to: *mut Context);
}
