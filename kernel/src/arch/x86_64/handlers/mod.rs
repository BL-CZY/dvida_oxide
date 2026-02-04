pub mod irq;
pub mod isr;

#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct InterruptNoErrcodeFrame {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct InterruptErrcodeFrame {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub err_code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[macro_export]
macro_rules! handler_inner_header {
    ($stack_frame:ident) => {
        if $stack_frame.cs & 0b111 == 0b11 {
            unsafe {
                asm!("swapgs");
            }
        }
    };
}

#[macro_export]
macro_rules! handler_wrapper_noerrcode {
    ($handler:ident) => {
    naked_asm!(
        r#"
        push rax
        mov rax, [rsp + 16]
        test al, 0b11

        jz 1f
        swapgs
        
        1:
        pop rax

        push r15
        push r14
        push r13
        push r12
        push r11
        push r10
        push r9
        push r8
        push rbp
        push rsi
        push rdi
        push rdx
        push rcx
        push rbx
        push rax

        mov rdi, rsp
        call {handler}

        pop rax
        pop rbx
        pop rcx
        pop rdx
        pop rdi
        pop rsi
        pop rbp
        pop r8
        pop r9
        pop r10
        pop r11
        pop r12
        pop r13
        pop r14
        pop r15
        
        push rax
        mov rax, [rsp + 16]
        test al, 0b11

        jz 2f
        swapgs
        
        2:
        pop rax

        iretq
    "#,
        handler = sym $handler,
    )
    };
}

#[macro_export]
macro_rules! handler_wrapper_errcode {
    ($handler:ident) => {
    naked_asm!(
        r#"
        push rax
        mov rax, [rsp + 24]
        test al, 0b11

        jz 1f 
        swapgs
        
        1:
        pop rax

        push r15
        push r14
        push r13
        push r12
        push r11
        push r10
        push r9
        push r8
        push rbp
        push rsi
        push rdi
        push rdx
        push rcx
        push rbx
        push rax

        mov rdi, rsp
        call {handler}

        pop rax
        pop rbx
        pop rcx
        pop rdx
        pop rdi
        pop rsi
        pop rbp
        pop r8
        pop r9
        pop r10
        pop r11
        pop r12
        pop r13
        pop r14
        pop r15

        push rax
        mov rax, [rsp + 24]
        test al, 0b11

        jz 2f
        swapgs
        
        2:
        pop rax

        add rsp, 8

        iretq
    "#,
        handler = sym $handler,
    )
    };
}
