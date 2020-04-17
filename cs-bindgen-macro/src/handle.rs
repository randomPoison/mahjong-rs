//! Utilities for generating the bindings for types that should be marshaled as a handle.

use crate::{describe_named_type, BindingStyle};
use proc_macro2::TokenStream;
use quote::*;
use syn::*;

pub fn quote_type_as_handle(ident: &Ident) -> syn::Result<TokenStream> {
    let drop_ident = format_drop_ident!(ident);
    let describe_fn = describe_named_type(ident, BindingStyle::Handle);

    Ok(quote! {
        // Implement `Abi` for the type and references to the type.

        impl cs_bindgen::abi::Abi for #ident {
            type Abi = *const Self;

            fn as_abi(&self) -> Self::Abi {
                self
            }

            fn into_abi(self) -> Self::Abi {
                std::boxed::Box::into_raw(std::boxed::Box::new(self))
            }

            unsafe fn from_abi(abi: Self::Abi) -> Self {
                *std::boxed::Box::from_raw(abi as *mut _)
            }
        }

        impl<'a> cs_bindgen::abi::Abi for &'a #ident {
            type Abi = *const #ident;

            fn as_abi(&self) -> Self::Abi {
                #ident::as_abi(self)
            }

            fn into_abi(self) -> Self::Abi {
                #ident::as_abi(self)
            }

            unsafe fn from_abi(abi: Self::Abi) -> Self {
                &*abi
            }
        }

        impl<'a> cs_bindgen::abi::Abi for &'a mut #ident {
            type Abi = *const #ident;

            fn as_abi(&self) -> Self::Abi {
                #ident::as_abi(self)
            }

            fn into_abi(self) -> Self::Abi {
                #ident::as_abi(self)
            }

            unsafe fn from_abi(abi: Self::Abi) -> Self {
                &mut *(abi as *mut _)
            }
        }

        // Export a function that describes the exported type.
        #describe_fn

        // Export a function that can be used for dropping an instance of the type.
        #[no_mangle]
        pub unsafe extern "C" fn #drop_ident(_: <#ident as cs_bindgen::abi::Abi>::Abi) {}
    })
}
