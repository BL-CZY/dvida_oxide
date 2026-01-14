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

    mov r15, qword [rdi + 0]
    mov r14, qword [rdi + 0x8]
    mov r13, qword [rdi + 0x10]
    mov r12, qword [rdi + 0x18]
    mov r11, qword [rdi + 0x20]         ; rflags
    mov r10, qword [rdi + 0x28]
    mov r9, qword [rdi + 0x30]
    mov r8, qword [rdi + 0x38]
    mov rsi, qword [rdi + 0x48]
    mov rdx, qword [rdi + 0x50]
    mov rcx, qword [rdi + 0x58]         ; rip
    mov rbx, qword [rdi + 0x60]
    mov rax, qword [rdi + 0x68]
    mov rbp, qword [rdi + 0x70]
    mov rsp, qword [rdi + 0x78]         ; pivot the stack
    mov rdi, qword [rdi + 0x40]

    swapgs
    sysretq

; rdi = stack frame with above layout
; rsi = page table
; rdx = long return frame defined in syscall.rs
resume_paused_thread:
    mov rax, rsi
    mov cr3, rax

    push qword [rdx + 0]
    push qword [rdx + 0x8]
    push qword [rdx + 0x10]
    push qword [rdx + 0x18]
    push qword [rdx + 0x20]

    mov r15, qword [rdi + 0]
    mov r14, qword [rdi + 0x8]
    mov r13, qword [rdi + 0x10]
    mov r12, qword [rdi + 0x18]
    mov r11, qword [rdi + 0x20]         
    mov r10, qword [rdi + 0x28]
    mov r9, qword [rdi + 0x30]
    mov r8, qword [rdi + 0x38]
    mov rsi, qword [rdi + 0x48]         ; rsi should not be used from now on
    mov rdx, qword [rdi + 0x50]         ; rdx should not be used from now on
    mov rcx, qword [rdi + 0x58]         
    mov rbx, qword [rdi + 0x60]
    mov rax, qword [rdi + 0x68]
    mov rbp, qword [rdi + 0x70]
    mov rdi, qword [rdi + 0x40]         ; rdi should not be used from now on
    
    swapgs
    iretq

