use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(AsEnvelope)]
pub fn derive_as_envelope(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics ::envelope::AsEnvelope for #name #ty_generics #where_clause {
            fn into_envelope(self) -> ::envelope::ResultEnvelope<Self> {
                ::envelope::ResultEnvelope::builder()
                    .result(::envelope::OperationResult::success(self))
                    .build()
                    .expect("Building envelope with valid data should not fail")
            }
        }
    };

    TokenStream::from(expanded)
}
