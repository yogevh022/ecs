use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(Component)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    quote! {
        impl crate::component::Component for #name {
            fn component_id() -> crate::component::ComponentId {
                static ID: std::sync::OnceLock<crate::component::ComponentId> =
                    std::sync::OnceLock::new();
                *ID.get_or_init(|| crate::component::registry().component_id_of::<Self>())
            }
        }
    }.into()
}