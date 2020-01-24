use cs_bindgen_shared::*;
use heck::*;
use proc_macro2::TokenStream;
use quote::*;
use std::{ffi::OsStr, fs, path::PathBuf, str};
use structopt::*;
use syn::*;
use wasmtime::*;

fn main() {
    let opt = Opt::from_args();

    let store = Store::default();

    let test_wasm = fs::read(&opt.input).expect("Couldn't read mahjong.wasm");
    let module = Module::new(&store, &test_wasm).expect("Failed to create WASM module");
    let instance = Instance::new(&store, &module, &[]).expect("Failed to create module instance");

    let len_fn = instance
        .find_export_by_name("__cs_bindgen_decl_len_generate_tileset_json")
        .expect("len fn not found")
        .func()
        .expect("len fn wasn't a fn???")
        .borrow();

    let decl_fn = instance
        .find_export_by_name("__cs_bindgen_decl_ptr_generate_tileset_json")
        .expect("decl fn not found")
        .func()
        .expect("decl fn wasn't a fn???")
        .borrow();

    let decl_ptr = decl_fn.call(&[]).expect("Failed to call decl fn")[0].unwrap_i32() as usize;
    let len = len_fn.call(&[]).expect("Failed to call len fn")[0].unwrap_i32() as usize;

    let memory = instance
        .find_export_by_name("memory")
        .expect("memory not found")
        .memory()
        .expect("memory wasn't a memory???")
        .borrow();

    // SAFETY: `Memory::data` is safe as long as we don't do anything that would
    // invalidate the reference while we're borrowing the memory. Specifically:
    //
    // * Explicitly calling `Memory::grow` (duh).
    // * Invoking a function in the module that contains the `memory.grow` instruction.
    //
    // That second one is the more critical one, because it means we have to make sure
    // we don't invoke *any* function in the module while borrowing the memory. For
    // our purposes that's fine, and we can probably write a safe wrapper function that
    // copies out the specified data so that we don't have to hold the borrow on the
    // memory.
    let memory_bytes = unsafe { memory.data() };

    let decl_bytes = &memory_bytes[decl_ptr..decl_ptr + len];

    let decl = str::from_utf8(decl_bytes).expect("decl not valid utf8");

    let bindgen_fn =
        serde_json::from_str::<BindgenFn>(&decl).expect("Failed to deserialize bindgen fn decl");

    // Generate the C# binding code.
    // ---------------------------------------------------------------------------------------------

    let dll_name = opt
        .input
        .file_stem()
        .and_then(OsStr::to_str)
        .expect("Unable to get name of wasm file");

    let class_name = format_ident!("{}", dll_name.to_camel_case());
    let entry_point = bindgen_fn.generated_name();
    let raw_binding = format_ident!("__{}", bindgen_fn.raw_ident().to_camel_case());
    let binding_return_ty = quote_binding_return_type(&bindgen_fn.ret);

    // If the function returns a string, generate an extra parameter binding for the
    // string's length.
    let out_len = match &bindgen_fn.ret {
        Some(Primitive::String) => quote! { out int length },
        _ => TokenStream::new(),
    };

    let wrapper_fn = quote_wrapper_fn(&bindgen_fn, &raw_binding);

    let result = quote! {
        using System;
        using System.Runtime.InteropServices;
        using System.Text;

        public class #class_name
        {
            [DllImport(
                #dll_name,
                EntryPoint = #entry_point,
                CallingConvention = CallingConvention.Cdecl)]
            private static extern #binding_return_ty #raw_binding(#out_len);

            [DllImport(
                #dll_name,
                EntryPoint = "__cs_bindgen_drop_string",
                CallingConvention = CallingConvention.Cdecl)]
            private static extern void DropString(IntPtr raw);

            #wrapper_fn
        }
    }
    .to_string();

    println!("{}", result);
}

fn quote_binding_return_type(return_ty: &Option<Primitive>) -> TokenStream {
    match return_ty {
        None => TokenStream::new(),
        Some(Primitive::String) => quote! { IntPtr },
        Some(Primitive::Char) => quote! { uint },
        Some(Primitive::I8) => quote! { sbyte },
        Some(Primitive::I16) => quote! { short },
        Some(Primitive::I32) => quote! { int },
        Some(Primitive::I64) => quote! { long },
        Some(Primitive::U8) => quote! { byte },
        Some(Primitive::U16) => quote! { ushort },
        Some(Primitive::U32) => quote! { uint },
        Some(Primitive::U64) => quote! { ulong },
        Some(Primitive::F32) => quote! { float },
        Some(Primitive::F64) => quote! { double },
        Some(Primitive::Bool) => quote! { byte },
    }
}

fn quote_return_type(return_ty: &Option<Primitive>) -> TokenStream {
    match return_ty {
        None => TokenStream::new(),
        Some(Primitive::String) => quote! { string },
        Some(Primitive::Char) => quote! { uint },
        Some(Primitive::I8) => quote! { sbyte },
        Some(Primitive::I16) => quote! { short },
        Some(Primitive::I32) => quote! { int },
        Some(Primitive::I64) => quote! { long },
        Some(Primitive::U8) => quote! { byte },
        Some(Primitive::U16) => quote! { ushort },
        Some(Primitive::U32) => quote! { uint },
        Some(Primitive::U64) => quote! { ulong },
        Some(Primitive::F32) => quote! { float },
        Some(Primitive::F64) => quote! { double },
        Some(Primitive::Bool) => quote! { bool },
    }
}

fn quote_wrapper_fn(bindgen_fn: &BindgenFn, raw_binding: &Ident) -> TokenStream {
    let cs_fn_name = format_ident!("{}", bindgen_fn.raw_ident().to_camel_case());
    let cs_return_ty = quote_return_type(&bindgen_fn.ret);

    // If the function returns a string, generate an extra parameter binding for the
    // string's length.
    let out_len = match &bindgen_fn.ret {
        Some(Primitive::String) => quote! { out var length },
        _ => TokenStream::new(),
    };

    let invoke_expr = match &bindgen_fn.ret {
        None => quote! { #raw_binding(); },

        Some(prim) => {
            let invoke_expr = quote! { var rawResult = #raw_binding(#out_len); };

            let result_expr = match prim {
                Primitive::String => quote! {
                    string result;
                    unsafe
                    {
                        result = Encoding.UTF8.GetString((byte*)rawResult, length);
                    }

                    DropString(rawResult);

                    return result;
                },

                Primitive::Bool => quote! {
                    return rawResult != 0;
                },

                _ => quote! { return rawResult },
            };

            quote! {
                #invoke_expr

                #result_expr
            }
        }
    };

    quote! {
        public static #cs_return_ty #cs_fn_name()
        {
            // TODO: Process args so they're ready to pass to the rust fn.

            #invoke_expr
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "cs-bindgen")]
struct Opt {
    #[structopt(parse(from_os_str))]
    input: PathBuf,
}
