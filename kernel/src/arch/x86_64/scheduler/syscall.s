.global syscall_handler_wrapper
.global resume_thread_from_syscall

section .text
syscall_handler_wrapper:
    swapgs                        ; swap out gs
    mov gs:[0x8], rsp             ; temporarily save rsp
    mov rsp, gs:[0x0]             ; pivot stack

    push qword gs:[0x10]          ; user rsp
    push rbp
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15

    mov rdi, rsp                  ; pass this entire struct to rdi
    call syscall_handler          ; call the handler 

; rdi = stack frame with above layout
; rsi = page table
resume_thread_from_syscall:
    mov rax, rsi
    mov cr3, rax

    mov r15, [rdi + 0]
    mov r14, [rdi + 0x8]
    mov r13, [rdi + 0x10]
    mov r12, [rdi + 0x18]
    mov r11, [rdi + 0x20]
    mov r10, [rdi + 0x28]
    mov r9, [rdi + 0x30]
    mov r8, [rdi + 0x38]
    mov rsi, [rdi + 0x48]
    mov rdx, [rdi + 0x50]
    mov rcx, [rdi + 0x58]
    mov rbx, [rdi + 0x60]
    mov rax, [rdi + 0x68]
    mov rbp, [rdi + 0x70]
    mov rsp, [rdi + 0x78]         ; pivot the stack
    mov rdi, [rdi + 0x40]

    swapgs
    sysretq
