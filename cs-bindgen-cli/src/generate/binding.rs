//! Utilities for generating the raw bindings to exported Rust items.
//!
//! This module provides the code generation for the C# declarations that bind to
//! Rust functions that are exported from the built dylib. Note that this
//! specifically refers to the *generated* functions, not the user defined
//! functions. This module also provides utilities for referencing the raw function
//! bindings in other parts of the code generation.
//!
//! In C#, the raw binding to an exported Rust function is a `static extern`
//! function, using the `[DllImport]` attribute to load the corresponding function
//! from the Rust dylib.

use crate::generate::{self, class, enumeration, strukt, TypeMap, STRING_SCHEMA};
use cs_bindgen_shared::{
    schematic::{Field, Schema, TypeName},
    BindingStyle, Export, FnArg, Repr,
};
use proc_macro2::TokenStream;
use quote::*;
use syn::{punctuated::Punctuated, token::Comma, Ident};

// TODO: For the below functions that generate identifiers based on a type name, we
// should use the fully-qualified `TypeName` instead of just a `&str` name. Right
// now, if two types with the same name in different modules are exported, the
// generated bindings will collide. We can avoid this by taking the module name into
// account when generating the idents. This will require some additional mangling
// logic, since the module paths include `::` characters, which aren't valid in C#
// identifiers.

/// Returns the identifier of the generating bindings class.
pub fn bindings_class_ident() -> Ident {
    format_ident!("__bindings")
}

/// The identifier of the from-raw conversion method.
///
/// This method is overloaded for every supported primitive and exported type, so it
/// can be used as a generic way to perform type conversion.
pub fn from_raw_fn_ident() -> Ident {
    format_ident!("__FromRaw")
}

/// The identifier of the into-raw conversion method.
///
/// This method is overloaded for every supported primitive and exported type, so it
/// can be used as a generic way to perform type conversion.
pub fn into_raw_fn_ident() -> Ident {
    format_ident!("__IntoRaw")
}

/// Generate the identifier for the raw type corresponding to the specified type.
///
/// When a user-defined type is marshaled by value, we generate a type that acts as
/// an FFI-safe "raw" representation for that type. When communicating with Rust, we
/// convert the C# representation of the type to-and-from the raw representation.
/// This function provides the canonical way to generate the name of the raw type
/// corresponding to any given exported Rust type.
pub fn raw_ident(type_name: &TypeName) -> Ident {
    format_ident!("__{}__Raw", type_name.name)
}

pub fn wrap_bindings(tokens: TokenStream) -> TokenStream {
    quote! {
        internal unsafe static partial class __bindings
        {
            #tokens
        }
    }
}

pub fn quote_raw_binding(export: &Export, dll_name: &str, types: &TypeMap) -> TokenStream {
    match export {
        Export::Fn(export) => {
            let args = quote_binding_args(&export.inputs, types);
            let return_ty = match &export.output {
                Some(output) => raw_type_from_repr(output, types),
                None => quote! { void },
            };

            quote_raw_fn_binding(&export.binding, return_ty, args.to_token_stream(), dll_name)
        }

        Export::Method(export) => {
            let return_ty = match &export.output {
                Some(output) => raw_type_from_repr(output, types),
                None => quote! { void },
            };

            // TODO: Unify input handling for raw bindings. It shouldn't be necessary to
            // manually insert the receiver. The current blocker is that schematic can't
            // represent reference types, so we can't generate a full list of inputs that
            // includes the receiver.
            let mut args = quote_binding_args(&export.inputs, types);
            if export.receiver.is_some() {
                let handle_type = class::quote_handle_ptr();
                args.insert(0, quote! { #handle_type self });
            }

            quote_raw_fn_binding(&export.binding, return_ty, args.to_token_stream(), dll_name)
        }

        // Generate the binding for the destructor for any named types that are marshaled
        // as handles.
        Export::Named(export) => match &export.binding_style {
            BindingStyle::Handle => class::quote_drop_fn(&export, dll_name),

            BindingStyle::Value(schema) => {
                let index_fn = quote_raw_fn_binding(
                    &export.index_fn,
                    raw_type_from_schema(schema, types),
                    quote! { RawSlice slice, UIntPtr index },
                    dll_name,
                );

                let convert_list_fn = quote_raw_fn_binding(
                    &export.convert_list_fn,
                    quote! { RawVec },
                    quote! { RawSlice raw },
                    dll_name,
                );

                let drop_vec_fn = quote_raw_fn_binding(
                    &export.drop_vec_fn,
                    quote! { void },
                    quote! { RawVec vec },
                    dll_name,
                );

                let from_raw = from_raw_fn_ident();
                let into_raw = into_raw_fn_ident();
                let ty = generate::quote_cs_type_for_schema(schema, types);
                let raw_repr = raw_type_from_schema(schema, types);
                let index_fn_name = format_ident!("{}", &*export.index_fn);
                let drop_vec_fn_name = format_ident!("{}", &*export.drop_vec_fn);
                let convert_list_fn_name = format_ident!("{}", &*export.convert_list_fn);

                let list_from_raw = quote! {
                    internal static void #from_raw(RawVec raw, out List<#ty> result)
                    {
                        result = raw.ToList<#raw_repr, #ty>(#index_fn_name, #from_raw);
                        #drop_vec_fn_name(raw);
                    }
                };

                let list_into_raw = quote! {
                    internal static void #into_raw(List<#ty> items, out RawVec result)
                    {
                        result = RawVec.FromList(
                            items,
                            item => {
                                #into_raw(item, out #raw_repr raw);
                                return raw;
                            },
                            #convert_list_fn_name);
                    }
                };

                quote! {
                    #index_fn
                    #convert_list_fn
                    #drop_vec_fn
                    #list_from_raw
                    #list_into_raw
                }
            }
        },
    }
}

/// Returns the raw type representation for for the given `repr`.
///
/// The raw representation is what is used in FFI calls:
///
/// * For primitive numeric types we use the corresponding C# numeric type.
/// * For `String` and `Vec` we use `RawVec`.
/// * For `str` and slices use `RawSlice`.
/// * For named types we look up the export definition to determine what raw repr to
///   use:
///   * Handle types are represented as a raw pointer (`IntPtr`, specifically).
///   * Value types have a corresponding raw struct.
///   * C-like enums that are passed by value use the numeric type of their
///     discriminant.
///
/// References to generated types are also prefixed with `global::` or the namespace
/// path as necessary.
pub fn raw_type_from_repr(repr: &Repr, types: &TypeMap) -> TokenStream {
    match repr {
        Repr::Unit => quote! { byte },
        Repr::Bool => quote! { byte },
        Repr::Char => quote! { uint },

        Repr::I8 => quote! { sbyte },
        Repr::I16 => quote! { short },
        Repr::I32 => quote! { int },
        Repr::I64 => quote! { long },
        Repr::ISize => quote! { IntPtr },

        Repr::U8 => quote! { byte },
        Repr::U16 => quote! { ushort },
        Repr::U32 => quote! { uint },
        Repr::U64 => quote! { ulong },
        Repr::USize => quote! { UIntPtr },

        Repr::F32 => quote! { float },
        Repr::F64 => quote! { double },

        Repr::Named(type_name) => {
            let export = types
                .get(&type_name)
                .unwrap_or_else(|| panic!("No export found for named type {:?}", type_name));

            match &export.binding_style {
                BindingStyle::Handle => class::quote_handle_ptr(),
                BindingStyle::Value(schema) => raw_type_from_schema(schema, types),
            }
        }

        // Pointer types are all marshalled as `IntPtr`.
        Repr::Box(_) | Repr::Ref(_) => quote! { IntPtr },

        Repr::Vec(_) => quote! { RawVec },
        Repr::Slice(_) => quote! { RawSlice },

        Repr::String => quote! { RawVec },
        Repr::Str => quote! { RawSlice },

        Repr::Array { .. } => todo!("Support arrays"),
        Repr::Option(_) => todo!("Support optional types"),
        Repr::Result { .. } => todo!("Support `Result`"),
    }
}

/// Generates the appropriate raw type name for the given type schema.
pub fn raw_type_from_schema(schema: &Schema, types: &TypeMap) -> TokenStream {
    match schema {
        Schema::Unit => quote! { byte },
        Schema::Bool => quote! { byte },
        Schema::Char => quote! { uint },

        Schema::I8 => quote! { sbyte },
        Schema::I16 => quote! { short },
        Schema::I32 => quote! { int },
        Schema::I64 => quote! { long },
        Schema::ISize => quote! { IntPtr },

        Schema::U8 => quote! { byte },
        Schema::U16 => quote! { ushort },
        Schema::U32 => quote! { uint },
        Schema::U64 => quote! { ulong },
        Schema::USize => quote! { UIntPtr },

        Schema::F32 => quote! { float },
        Schema::F64 => quote! { double },

        // NOTE: The `unwrap` here is valid because `String` is a built-in type and so
        // describing it will never fail.
        //
        // TODO: Directly compare the type names once the `Describe` trait has an associated
        // constant for type names.
        Schema::String(_) => {
            if schema == &*STRING_SCHEMA {
                quote! { RawVec }
            } else {
                todo!("Handle unknown custom string types")
            }
        }

        Schema::Str => quote! { RawSlice },

        Schema::Enum(schema) => {
            let export = types
                .get(&schema.name)
                .unwrap_or_else(|| panic!("No export found for named type {:?}", &schema.name));

            // There are three possible raw representations for an exported enum:
            //
            // * Enums that are marshalled as handles are represented as the raw handle pointer
            //   type (`IntPtr`).
            // * Data-carrying enums have an associate struct that represents its raw type.
            // * C-like enums are marshalled directly as an integer value.
            if matches!(export.binding_style, BindingStyle::Handle) {
                class::quote_handle_ptr()
            } else if schema.has_data() {
                named_type_raw_reference(&schema.name)
            } else {
                enumeration::quote_discriminant_type(schema)
            }
        }

        Schema::Struct(_)
        | Schema::UnitStruct(_)
        | Schema::NewtypeStruct(_)
        | Schema::TupleStruct(_) => {
            // NOTE: The `unwrap` here is valid because all of the struct-like variants are
            // guaranteed to have a type name. If this panic, that indicates a bug in the
            // schematic crate.
            let type_name = schema.type_name().unwrap();

            let export = types
                .get(type_name)
                .unwrap_or_else(|| panic!("No export found for named type {:?}", type_name));

            // Determine the raw representation based on the marshaling style.
            if matches!(export.binding_style, BindingStyle::Handle) {
                class::quote_handle_ptr()
            } else {
                named_type_raw_reference(type_name)
            }
        }

        Schema::Array(_) => todo!("Support passing fixed-size arrays"),

        Schema::Slice(_) => quote! { RawSlice },

        Schema::Seq(schema) => {
            if schema.name.name == "Vec" && schema.name.module == "alloc::vec" {
                quote! { RawVec }
            } else {
                todo!("Handle unknown sequence types")
            }
        }

        // TODO: Add support for collection types.
        Schema::Option(_) | Schema::Tuple(_) | Schema::Map { .. } => {
            todo!("Generate argument binding")
        }

        Schema::I128 | Schema::U128 => {
            unreachable!("Invalid types should have already been handled")
        }
    }
}

/// Generates the field definitions for the raw struct representation of an exported
/// Rust type.
pub fn raw_struct_fields(fields: &[Field<'_>], types: &TypeMap) -> TokenStream {
    let field_name = fields
        .iter()
        .enumerate()
        .map(|(index, field)| strukt::field_ident(field.name, index));

    let field_ty = fields
        .iter()
        .map(|field| raw_type_from_schema(&field.schema, types));

    quote! {
        #(
            internal #field_ty #field_name;
        )*
    }
}

fn quote_binding_args<'a>(inputs: &[FnArg], types: &TypeMap<'_>) -> Punctuated<TokenStream, Comma> {
    inputs
        .iter()
        .map(|arg| {
            let ident = format_ident!("{}", &*arg.name);
            let ty = raw_type_from_repr(&arg.repr, types);
            quote! { #ty #ident }
        })
        .collect()
}

fn quote_raw_fn_binding(
    entry_point: &str,
    return_ty: TokenStream,
    args: TokenStream,
    dll: &str,
) -> TokenStream {
    let fn_name = format_ident!("{}", entry_point);
    quote! {
        [DllImport(
            #dll,
            EntryPoint = #entry_point,
            CallingConvention = CallingConvention.Cdecl)]
        internal static extern #return_ty #fn_name(#args);
    }
}

fn named_type_raw_reference(type_name: &TypeName) -> TokenStream {
    let ident = raw_ident(type_name);
    quote! {
        global::#ident
    }
}
