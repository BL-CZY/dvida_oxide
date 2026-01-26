#![no_std]

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Expr, parse_macro_input};

#[proc_macro]
pub fn ahci_interrupt_handler_template(_stream: TokenStream) -> TokenStream {
    let mut final_tokens = quote! {};

    for idx in 0..8 as usize {
        let handler_wrapper_name = format_ident!("ahci_interrupt_handler_{}", idx);
        let handler_inner_name = format_ident!("ahci_interrupt_handler_inner_{}", idx);

        final_tokens.extend(quote! {
            paste::paste! {
                extern "C" fn #handler_inner_name(_stack_frame: InterruptNoErrcodeFrame) {
                    ahci_interrupt_handler_by_idx(#idx);
                }

                #[unsafe(naked)]
                pub extern "x86-interrupt" fn #handler_wrapper_name(_stack_frame: InterruptStackFrame) {
                    handler_wrapper_noerrcode!(#handler_inner_name)
                }
            }
        });
    }

    final_tokens.into()
}

#[proc_macro]
pub fn idt_ahci(stream: TokenStream) -> TokenStream {
    let base = parse_macro_input!(stream as Expr);

    let mut final_tokens = quote! {};

    for idx in 0..8 as u8 {
        let handler_name = format_ident!("ahci_interrupt_handler_{}", idx);
        final_tokens.extend(quote! {
            idt[#base + #idx].set_handler_fn(irq::#handler_name);
        });
    }

    final_tokens.into()
}
