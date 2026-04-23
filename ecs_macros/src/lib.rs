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
    }
    .into()
}

#[proc_macro]
pub fn impl_queryable_variadic_up_to(input: TokenStream) -> TokenStream {
    let count: usize = parse_macro_input!(input as syn::LitInt)
        .base10_parse()
        .unwrap();

    let mut impls = Vec::with_capacity(count);
    let mut types = Vec::with_capacity(count);
    let mut indices = Vec::with_capacity(count);
    let mut blob_vecs = Vec::with_capacity(count);

    for n in 1..=count {
        types.clear();
        indices.clear();
        blob_vecs.clear();

        types.extend((0..n).map(|i| quote::format_ident!("T{}", i)));
        indices.extend((0..n).map(syn::Index::from));
        blob_vecs.extend((0..n).map(|_| quote! { *mut BlobVec }));

        impls.push(quote! {
                impl<#(#types: Component),*> Queryable for (#(#types,)*) {
                    type Key = ArchetypeKey;
                    type IterTuple<'a> = (#(&'a mut #types,)*);
                    type ColumnTuple = (#(#blob_vecs,)*);

                    #[inline]
                    fn key() -> Self::Key {
                        let mut key = ArchetypeKey::EMPTY;
                        #(key = key.with::<#types>();)*
                        key
                    }

                    #[inline]
                    fn fetch_row<'a>(columns: Self::ColumnTuple, index: usize) -> Self::IterTuple<'a> {
                        unsafe {
                            (#(
                                // SAFETY: index < end, enforced at construction
                                (*columns.#indices).get_unchecked_mut(index),
                            )*)
                        }
                    }

                    #[inline]
                    fn fetch_columns(arch: &mut Archetype) -> Self::ColumnTuple {
                        (#(
                            arch.get_column_mut::<#types>() as *mut BlobVec,
                        )*)
                    }
                }
        })
    }

    quote! {
        #(#impls)*
    }
    .into()
}
