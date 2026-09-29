.intel_syntax noprefix
.text
.globl {P}main
{P}main:
    push rbp
    mov rbp, rsp
    push rbx
    push r12
    push r13
    push r14
    push r15
    sub rsp, 8
    mov r13, rdi
    mov r14, rsi
    lea rdi, [rip + burn_meta]
    mov rsi, qword ptr [rip + burn_meta_len]
    lea rdx, [rip + {P}burn_globals]
    mov rcx, qword ptr [rip + burn_nglobals]
    mov r8, rbp
    lea r9, [rip + {P}burn_call_trampoline]
    mov r12, rsp
    and rsp, -16
    call {P}burn_rt_init{PLT}
    mov rsp, r12
    mov rdi, r13
    mov rsi, r14
    mov r12, rsp
    and rsp, -16
    call {P}burn_rt_set_args{PLT}
    mov rsp, r12
{PRE_ENTRY}    call burn_entry
    xor edi, edi
    mov r12, rsp
    and rsp, -16
    call {P}burn_rt_exit{PLT}
    mov rsp, r12
    add rsp, 8
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    ret

.globl {P}burn_call_trampoline
{P}burn_call_trampoline:
    push rbp
    mov rbp, rsp
    push rbx
    push r12
    push r13
    push r14
    push r15
    sub rsp, 8
    mov rax, rdi
    mov r12, rdx
.Ltramp_loop:
    test r12, r12
    jz .Ltramp_done
    dec r12
    push qword ptr [rsi + r12*8]
    jmp .Ltramp_loop
.Ltramp_done:
    call rax
    lea rsp, [rbp - 40]
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    ret
