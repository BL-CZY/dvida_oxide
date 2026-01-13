.global syscall_handler_wrapper

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
    

