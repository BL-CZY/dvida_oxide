#![no_std]

use dvida_serialize::{DvSerErr, DvSerialize};

extern crate proc_macro;
use proc_macro::TokenStream;

use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident, parse_macro_input};

fn make_error(ident: &Ident, msg: &str) -> TokenStream {
    return syn::Error::new_spanned(&ident, msg)
        .to_compile_error()
        .into();
}

#[proc_macro_derive(DvSerialize)]
pub fn derive_dv_serialize(input: TokenStream) -> TokenStream {
    let DeriveInput {
        attrs,
        vis,
        ident,
        generics,
        data,
    } = parse_macro_input!(input as DeriveInput);

    let data_struct = if let Data::Struct(data_struct) = data {
        data_struct
    } else {
        return make_error(&ident, "Only structs are supported");
    };

    for field in data_struct.fields.iter() {
        match field.ident {
            Some(name) => {}
            None => continue,
        }
    }
}
