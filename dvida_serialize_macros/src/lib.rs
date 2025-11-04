#![no_std]

use dvida_serialize::{DvSerErr, DvSerialize};

extern crate proc_macro;
use proc_macro::TokenStream;

use quote::quote;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(DvSerialize)]
pub fn derive_dv_serialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let generics = &input.generics;

    // Input: struct Foo<T: Clone, U> where U: Debug { ... }
    // Generates: impl<T: Clone, U> MyTrait for Foo<T, U> where U: Debug { ... }
    //            ^^^^^ impl_generics   ^^^^ ty_generics  ^^^^^^^^^^^^^^ where_clause

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics for #name #ty_generics #where_clause {
            fn serialize(&self, endianness: Endianness) {}
        }
    };

    TokenStream::from(expanded)
}
