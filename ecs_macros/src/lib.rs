use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(Component)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let id_ty = quote!(crate::component::ComponentId);

    quote! {
        impl crate::component::Component for #name {
            fn component_id() -> #id_ty {
                static ID: std::sync::OnceLock<#id_ty> = std::sync::OnceLock::new();
                *ID.get_or_init(|| crate::component::get().id_of::<Self>())
            }
        }
    }
    .into()
}

#[proc_macro_derive(Event)]
pub fn derive_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let id_ty = quote!(crate::event::EventId);

    quote! {
        impl crate::event::Event for #name {
            fn event_id() -> #id_ty {
                static ID: std::sync::OnceLock<#id_ty> = std::sync::OnceLock::new();
                0 // fixme
                // *ID.get_or_init(|| crate::event::get().id_of::<Self>())
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
                    type IterTuple<'a> = (#(&'a mut #types,)*);
                    type ColumnTuple = (#(#blob_vecs,)*);

                    #[inline]
                    fn key() -> ArchetypeKey {
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

#[proc_macro]
pub fn impl_component_group_variadic_up_to(input: TokenStream) -> TokenStream {
    let count: usize = parse_macro_input!(input as syn::LitInt)
        .base10_parse()
        .unwrap();

    let mut impls = Vec::with_capacity(count);
    let mut types = Vec::with_capacity(count);

    for n in 1..=count {
        types.clear();
        types.extend((0..n).map(|i| quote::format_ident!("T{}", i)));

        impls.push(quote! {
            impl<#(#types: Component),*> ComponentGroup for (#(#types,)*) {
                fn key() -> ArchetypeKey {
                    let mut key = ArchetypeKey::EMPTY;
                    #(key = key.with::<#types>();)*
                    key
                }
            }
        });
    }

    quote! {
        #(#impls)*
    }
    .into()
}

#[proc_macro]
pub fn impl_into_system_variadic_up_to(input: TokenStream) -> TokenStream {
    let count: usize = parse_macro_input!(input as syn::LitInt)
        .base10_parse()
        .unwrap();

    let mut impls = Vec::with_capacity(count);
    let mut types = Vec::with_capacity(count);
    let mut states = Vec::with_capacity(count);
    let mut params = Vec::with_capacity(count);

    for n in 1..=count {
        types.clear();
        states.clear();
        params.clear();

        types.extend((0..n).map(|i| quote::format_ident!("T{}", i)));
        states.extend((0..n).map(|i| quote::format_ident!("s{}", i)));
        params.extend((0..n).map(|i| quote::format_ident!("p{}", i)));

        impls.push(quote! {
                impl<F, #(#types,)*> IntoSystem<(#(#types,)*)> for F
                where
                    F: FnMut(#(#types,)*) + FnMut(#(#types::Item<'_>,)*) + 'static,
                    #(#types: SysParam),*
                {
                    fn into_system(mut self, ecs: &mut Ecs) -> Box<dyn FnMut(*mut Ecs)> {
                        #(let mut #states = #types::init(ecs);)*
                        Box::new(move |ecs| {
                            #(let #params = #types::fetch(ecs, &mut #states);)*
                            self(#(#params,)*)
                        })
                    }
                }
        })
    }

    let expanded = quote! {
        #(#impls)*
    };
    expanded.into()
}
