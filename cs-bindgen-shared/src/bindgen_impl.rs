use crate::BindgenFn;
use serde::*;
use syn::{spanned::Spanned, Error, ImplItem, Type};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindgenImpl {
    pub ty_ident: String,
    pub methods: Vec<BindgenFn>,
}

impl BindgenImpl {
    pub fn from_item(item: syn::ItemImpl) -> syn::Result<Self> {
        if let Some((_, path, _)) = item.trait_ {
            return Err(Error::new(
                path.span(),
                "Trait impls are not yet supported with `#[cs_bindgen]`",
            ));
        }

        if !item.generics.params.is_empty() {
            return Err(Error::new(
                item.generics.span(),
                "Generic impls are not not supported with `#[cs_bindgen]`",
            ));
        }

        let ty_ident = if let Type::Path(path) = *item.self_ty {
            path.path
                .get_ident()
                .map(|ident| ident.to_string())
                .ok_or(Error::new(
                    path.span(),
                    "Self type not supported in impl for `#[cs_bindgen]`",
                ))?
        } else {
            return Err(Error::new(
                item.self_ty.span(),
                "Impls for this type of item are not supported by `#[cs_bindgen]`",
            ));
        };

        let methods = item
            .items
            .iter()
            .filter_map(|item| match item {
                ImplItem::Method(item) => Some(&item.sig),
                _ => None,
            })
            .map(BindgenFn::from_signature)
            .collect::<syn::Result<_>>()?;

        Ok(Self { ty_ident, methods })
    }
}