use crate::generate::{binding, class, quote_cs_type, quote_primitive_type, TypeMap};
use cs_bindgen_shared::{schematic::Enum, schematic::Variant, BindingStyle, NamedType};
use heck::*;
use proc_macro2::TokenStream;
use quote::*;

pub fn quote_enum_binding(export: &NamedType, schema: &Enum, types: &TypeMap) -> TokenStream {
    // Determine if we're dealing with a simple (C-like) enum or one with fields.
    if schema.has_data() {
        quote_complex_enum_binding(export, schema, types)
    } else {
        quote_simple_enum_binding(export, schema)
    }
}

pub fn quote_type_reference(export: &NamedType, schema: &Enum) -> TokenStream {
    if export.binding_style == BindingStyle::Value && schema.has_data() {
        format_ident!("I{}", &*export.name).into_token_stream()
    } else {
        format_ident!("{}", &*export.name).into_token_stream()
    }
}

/// Quotes the name of the generated C# type for the exported enum.
///
/// There are three possible raw representations for an enum:
///
/// * For C-like enums, the raw representation is the integer type used for the
///   enum's discriminant.
/// * For data-carrying enums that are marshaled by value, the raw representation is
/// * For enums that are marshalled as handles, the raw representation is just the
///   handle pointer type (`void*`).
pub fn quote_raw_type_reference(export: &NamedType, schema: &Enum) -> TokenStream {
    match export.binding_style {
        BindingStyle::Value => {
            if schema.has_data() {
                let union_ty = format_ident!("{}__Raw", &*export.name);
                quote! {
                    RawEnum<#union_ty>
                }
            } else {
                quote_discriminant_type(schema)
            }
        }

        BindingStyle::Handle => class::quote_handle_ptr(),
    }
}

/// Quotes the appropriate discriminant type for the specified enum type.
///
/// The generated type is the type use to represent the raw discriminant when
/// communicating with Rust. On the C# side, C-like enums are always represented as
/// `int` under the hood, and complex enums don't have a specific discriminant since
/// they are represented using an interface.
pub fn quote_discriminant_type(schema: &Enum) -> TokenStream {
    schema
        .repr
        .map(quote_primitive_type)
        .unwrap_or_else(|| quote! { IntPtr })
}

fn quote_simple_enum_binding(export: &NamedType, schema: &Enum) -> TokenStream {
    let ident = format_ident!("{}", &*export.name);
    let variants = schema.variants.iter().map(|variant| {
        let (name, discriminant) = match variant {
            Variant::Unit { name, discriminant } => (name, discriminant),

            _ => panic!("Simple enum can only have unit variants"),
        };

        let variant_ident = format_ident!("{}", &**name);
        let discriminant = match discriminant {
            Some(discriminant) => {
                let lit = syn::parse_str::<syn::Expr>(&discriminant.to_string())
                    .expect("Failed to parse discriminant as a `LitInt`");
                quote! { = #lit }
            }
            None => TokenStream::new(),
        };

        quote! {
            #variant_ident #discriminant
        }
    });

    quote! {
        public enum #ident {
            #( #variants ),*
        }
    }
}

fn quote_complex_enum_binding(export: &NamedType, schema: &Enum, types: &TypeMap) -> TokenStream {
    assert_eq!(
        export.binding_style,
        BindingStyle::Value,
        "Right now we only support exporting complex enums by value"
    );

    let interface = format_ident!("I{}", &*export.name);

    let arg_variants = schema.variants.iter().map(|variant| {
        let binding_ty = format_ident!("{}__RawArg", variant.name());
        let name = format_ident!("{}", variant.name());

        quote! {
            #binding_ty #name
        }
    });

    let return_variants = schema.variants.iter().map(|variant| {
        let binding_ty = format_ident!("{}__RawReturn", variant.name());
        let name = format_ident!("{}", variant.name());

        quote! {
            #binding_ty #name
        }
    });

    let variant_structs = schema.variants.iter().map(|variant| {
        let ident = format_ident!("{}", variant.name());
        let arg_binding_ident = format_ident!("{}__RawArg", variant.name());
        let return_binding_ident = format_ident!("{}__RawReturn", variant.name());

        let fields = variant
            .fields()
            .enumerate()
            .map(|(index, field)| {
                let field_ident = field
                    .name
                    .as_ref()
                    .map(|name| format_ident!("{}", name.to_camel_case()))
                    .unwrap_or_else(|| format_ident!("Element{}", index));

                (field_ident, field.schema)

                // (fields, binding_fields)
            })
            .collect::<Vec<_>>();

        let struct_fields = fields.iter().map(|(field_ident, schema)| {
            let ty = quote_cs_type(schema, types);
            quote! {
                public #ty #field_ident
            }
        });

        let arg_binding_fields = fields.iter().map(|(field_ident, schema)| {
            let binding_ty = binding::quote_type_binding(schema, types);

            quote! {
                internal #binding_ty #field_ident
            }
        });

        let return_binding_fields = fields.iter().map(|(field_ident, schema)| {
            let binding_ty = binding::quote_type_binding(schema, types);

            quote! {
                internal #binding_ty #field_ident
            }
        });

        quote! {
            public struct #ident : #interface
            {
                #( #struct_fields; )*
            }

            [StructLayout(LayoutKind.Sequential)]
            internal struct #arg_binding_ident
            {
                #( #arg_binding_fields; )*
            }

            [StructLayout(LayoutKind.Sequential)]
            internal struct #return_binding_ident
            {
                #( #return_binding_fields; )*
            }
        }
    });

    let arg_union = format_ident!("{}__RawArg", &*export.name);
    let return_union = format_ident!("{}__RawReturn", &*export.name);

    quote! {
        // Generate an interface for the enum.
        public interface #interface {}

        // Generate the struct declarations for each variant of the enum.
        #( #variant_structs )*

        // Generate the binding "unions" for args/returns.
        [StructLayout(LayoutKind.Explicit)]
        internal struct #arg_union
        {
            #(
                [FieldOffset(0)]
                #arg_variants;
            )*
        }

        [StructLayout(LayoutKind.Explicit)]
        internal struct #return_union
        {
            #(
                [FieldOffset(0)]
                #return_variants;
            )*
        }
    }
}
