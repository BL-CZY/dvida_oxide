extern crate proc_macro;
use proc_macro::TokenStream;

use quote::quote;
use syn::{Data, DeriveInput, Field, Ident, Type, parse_macro_input};

fn make_error(ident: &Ident, msg: &str) -> TokenStream {
    return syn::Error::new_spanned(&ident, msg)
        .to_compile_error()
        .into();
}

#[proc_macro_derive(DvDeSer)]
pub fn derive_dv_deser(input: TokenStream) -> TokenStream {
    let DeriveInput {
        attrs: _,
        vis: _,
        ident,
        generics,
        data,
    } = parse_macro_input!(input as DeriveInput);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    // Input: struct Foo<T: Clone, U> where U: Debug { ... }
    // Generates: impl<T: Clone, U> MyTrait for Foo<T, U> where U: Debug { ... }
    //            ^^^^^ impl_generics   ^^^^ ty_generics  ^^^^^^^^^^^^^^ where_clause

    let data_struct = if let Data::Struct(data_struct) = data {
        data_struct
    } else {
        return make_error(&ident, "Only structs are supported");
    };

    let names: Vec<Ident> = data_struct
        .fields
        .iter()
        .filter_map(|f| f.ident.clone())
        .collect();

    let fields: Vec<&Field> = data_struct
        .fields
        .iter()
        .filter(|f| match &f.ident {
            Some(_) => true,
            None => false,
        })
        .collect();

    let types: Vec<&Type> = fields.iter().map(|f| &f.ty).collect();

    let expanded = quote! {
        impl #impl_generics DvSerialize for #ident #ty_generics #where_clause {
            fn serialize(&self, endianness: Endianness, target: &mut [u8]) -> Result<usize, DvSerErr> {
                let mut acc: usize = 0;

                #( acc += self.#names.serialize(endianness, &mut target[acc..])?; )*

                Ok(acc)
            }
        }

        impl #impl_generics DvDeserialize for #ident #ty_generics #where_clause {
            fn deserialize(endianness: Endianness, input: &[u8]) -> Result<(Self, usize), DvDeErr>
            where
                Self: Sized,
            {
                let mut acc: usize = 0;

                #(

                let (#names, written) = #types::deserialize(endianness, &input[acc..])?;
                acc += written;

                )*

                Ok((Self { #( #names ),* }, acc))
            }

        }
    };

    expanded.into()
}
