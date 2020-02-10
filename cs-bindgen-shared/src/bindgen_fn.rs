use crate::{FnArg, Primitive, ReturnType};
use proc_macro2::Span;
use serde::*;
use syn::{spanned::Spanned, *};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindgenFn {
    ident: String,

    pub receiver: Option<Receiver>,

    // TODO: Preserve variable names for function arguments or we won't be able to
    // generate code for functions that actually have args.
    pub args: Vec<FnArg>,

    pub ret: ReturnType,
}

impl BindgenFn {
    pub fn from_signature(signature: &Signature) -> syn::Result<Self> {
        // Generate an error for async functions.
        if let Some(asyncness) = signature.asyncness {
            return Err(Error::new(
                asyncness.span(),
                "Async functions are not supported with `#[cs_bindgen]`",
            ));
        }

        let receiver = signature.receiver().map(Receiver::from_syn).transpose()?;

        // Parse function arguments.
        let args = signature
            .inputs
            .iter()
            .enumerate()
            .map(|(index, arg)| match arg {
                // Reject any functions that take some form of `self`. We'll eventually be able to
                // support these by marking entire `impl` blocks with `#[cs_bindgen]`, but for now
                // we only support free functions.
                syn::FnArg::Receiver(_) => Err(syn::Error::new(
                    arg.span(),
                    "Methods are not supported, only free functions",
                )),

                syn::FnArg::Typed(pat) => {
                    // If the argument isn't declared with a normal identifier, we construct one so
                    // that we have a valid identifier to use in the generated functions.
                    let ident = match &*pat.pat {
                        Pat::Ident(pat_ident) => pat_ident.ident.to_string(),
                        _ => format!("__arg{}", index),
                    };

                    let ty = Primitive::from_type(&pat.ty).ok_or(syn::Error::new(
                        pat.ty.span(),
                        "Unknown argument type, only primitives are supported",
                    ))?;

                    Ok(FnArg::new(ident, ty))
                }
            })
            .collect::<syn::Result<_>>()?;

        Ok(Self {
            ident: signature.ident.to_string(),
            receiver,
            args,
            ret: ReturnType::from_syn(&signature.output)?,
        })
    }

    /// Returns the raw string representation of the function's name.
    ///
    /// Be careful about how this function is used. If the returned value is quasi-quoted
    /// directly, it'll generate a string in the output rather than an identifier. Use
    /// `ident` to get a proper `Ident` for use in quasi-quoting, or use `format_ident!`
    /// to concatenate this value into a valid `Ident`.
    pub fn raw_ident(&self) -> &str {
        &self.ident
    }

    /// Returns the name of the function as an identifier suitable for quasi-quoting.
    pub fn ident(&self) -> Ident {
        Ident::new(&self.ident, Span::call_site())
    }

    pub fn generated_name(&self) -> String {
        format!("__cs_bindgen_generated_{}", self.ident)
    }

    pub fn generated_ident(&self) -> Ident {
        Ident::new(&self.generated_name(), Span::call_site())
    }

    pub fn generated_method_name(&self, self_ty: &str) -> String {
        format!("__cs_bindgen_generated__{}__{}", self_ty, self.ident)
    }

    pub fn generated_method_ident(&self, self_ty: &str) -> Ident {
        Ident::new(&self.generated_method_name(self_ty), Span::call_site())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Receiver {
    Ref,
    RefMut,
    Value,
}

impl Receiver {
    pub fn from_syn(arg: &syn::FnArg) -> syn::Result<Self> {
        match arg {
            syn::FnArg::Receiver(recv) => Ok({
                if recv.reference.is_some() {
                    if recv.mutability.is_some() {
                        Receiver::RefMut
                    } else {
                        Receiver::Ref
                    }
                } else {
                    Receiver::Value
                }
            }),

            syn::FnArg::Typed(_) => Err(Error::new(
                arg.span(),
                "Invalid receiver for `#[cs_bindgen]` method",
            )),
        }
    }
}
