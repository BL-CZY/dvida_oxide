use crate::arch::x86_64::memory::frame_allocator::setup_stack;

pub mod irq;
pub mod isr;

pub const RSP0_STACK_GUARD_PAGE: u64 = 0xFFFF_FF02_0000_0000;
pub const RSP0_STACK_LENGTH: u64 = 16;

pub fn setup_rsp0_stack() {
    setup_stack(RSP0_STACK_GUARD_PAGE, RSP0_STACK_LENGTH);
}

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
macro_rules! handler_wrapper {
    ($handler:ident) => {
    naked_asm!(
        r#"
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
        iretq
    "#,
        handler = sym $handler,
    )
    };
}
