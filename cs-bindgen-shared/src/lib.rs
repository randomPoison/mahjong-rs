use derive_more::From;
use serde::*;
use std::borrow::Cow;

// Re-export schematic so that dependent crates don't need to directly depend on it.
pub use schematic;
pub use schematic::Schema;

pub fn serialize_export<E: Into<Export>>(export: E) -> String {
    let export = export.into();
    serde_json::to_string(&export).expect("Failed to serialize export")
}

/// An item exported from the Rust as a language binding.
#[derive(Debug, Clone, From, Serialize, Deserialize)]
pub enum Export {
    Fn(Func),
    Method(Method),
    Named(NamedType),
}

/// A free function exported from the Rust lib.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Func {
    /// The original name of the function, as declared in the Rust source code.
    ///
    /// The function with this name cannot be called directly in the built lib. The
    /// value of `binding` specifies the name of the generated binding function.
    pub name: Cow<'static, str>,

    /// The name of the generated binding function.
    ///
    /// This is the exported function that is directly accessible in the generated
    /// lib. This name isn't meant to be user-facing, and should only be used
    /// internally by the generated language bindings to call into the lib. For the
    /// "true" name of the function, see `name`.
    pub binding: Cow<'static, str>,

    /// The argument types for the function.
    ///
    /// Note that these are the types of the original function, NOT the generated
    /// binding function.
    pub inputs: Vec<(Cow<'static, str>, Schema)>,

    /// The return type of the function.
    ///
    /// Note that this is the return type of the original function, NOT the generated
    /// binding function.
    pub output: Option<Schema>,
}

impl Func {
    /// Returns an iterator over the inputs to the function.
    ///
    /// The argument names are automatically deref'd from `Cow<str>` to `&str` for
    /// convenience. If direct access to the `Cow<str>` is needed, the `inputs` field
    /// can be used directly.
    pub fn inputs(&self) -> impl Iterator<Item = (&str, &Schema)> + Clone {
        self.inputs.iter().map(|(name, schema)| (&**name, schema))
    }
}

/// A user-defined type (i.e. a struct or an enum).
///
/// Both structs and enums are exported as "named types", since there are a number
/// of configuration options that are shared for all exported types. To determine
/// the full details of the exported type, examine the include `schema`.
///
/// An exported name type can only be a struct or an enum, as exporting unions is
/// not supported.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NamedType {
    pub name: Cow<'static, str>,
    pub binding_style: BindingStyle,
    pub schema: Schema,
}

#[derive(Debug, Clone, From, Serialize, Deserialize)]
pub struct Method {
    pub name: Cow<'static, str>,
    pub binding: Cow<'static, str>,
    pub self_type: Schema,
    pub receiver: Option<ReceiverStyle>,
    pub inputs: Vec<(Cow<'static, str>, Schema)>,
    pub output: Option<Schema>,
}

impl Method {
    pub fn inputs(&self) -> impl Iterator<Item = (&str, &Schema)> + Clone {
        self.inputs.iter().map(|(name, schema)| (&**name, schema))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReceiverStyle {
    Move,
    Ref,
    RefMut,
}

/// The style of binding generated for an exported type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BindingStyle {
    /// The type is exported as a class wrapping an opaque handle.
    Handle,

    /// Values of the type are marshalled directly into C# values.
    Value,
}
