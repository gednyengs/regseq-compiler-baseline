//! C type / declarator / declaration AST (ISO C11 6.7).
//!
//! Covers the Phase-7C subset needed by current Linux emission: named
//! scalar types (`u32`, `size_t`, ...), pointers (with optional `const`
//! qualifier), fixed-size arrays, `struct` tag references, and typedef
//! name references. Full declarator grammar (function-pointer types,
//! nested declarators, variadic parameter lists) is deferred until a
//! concrete emission site demands it.

use super::initializers::CInitializer;

/// A C type expression (ISO C11 6.7.2 type-specifier + 6.7.6
/// abstract-declarator).
#[derive(Debug, Clone)]
pub enum CType {
    /// Scalar or keyword type name (e.g. `int`, `u32`, `void`, `size_t`).
    Named(String),
    /// Pointer: `<inner> *` with optional top-level `const` qualifier
    /// (6.7.6.1 pointer).
    Pointer {
        /// Pointee type.
        inner: Box<CType>,
        /// When true, renders as `const <inner> *`.
        const_qualified: bool,
    },
    /// Fixed-size array: `<inner>[<size>]` (6.7.6.2 array).
    /// `size` of `None` renders as `[]`.
    Array {
        /// Element type.
        inner: Box<CType>,
        /// Optional compile-time array size.
        size: Option<usize>,
    },
    /// `struct <tag>` reference (6.7.2.1 struct-or-union-specifier).
    Struct(String),
    /// Typedef name reference (6.7.8 typedef-name).
    Typedef(String),
}

impl CType {
    /// Render the type as a complete type-name suitable for cast
    /// expressions and standalone uses (e.g. inside `(type)expr`). For
    /// declarations where a declarator name must appear between the
    /// specifier and any array suffix, prefer
    /// [`CDeclaration::render_into`], which handles the split.
    pub fn render_into(&self, out: &mut String) {
        match self {
            Self::Named(name) => out.push_str(name),
            Self::Pointer {
                inner,
                const_qualified,
            } => {
                if *const_qualified {
                    out.push_str("const ");
                }
                inner.render_into(out);
                if !out.ends_with('*') {
                    out.push(' ');
                }
                out.push('*');
            }
            Self::Array { inner, size } => {
                inner.render_into(out);
                match size {
                    Some(n) => out.push_str(&format!("[{}]", n)),
                    None => out.push_str("[]"),
                }
            }
            Self::Struct(tag) => {
                out.push_str("struct ");
                out.push_str(tag);
            }
            Self::Typedef(name) => out.push_str(name),
        }
    }
}

/// Storage-class specifier (ISO C11 6.7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CStorageClass {
    /// No storage-class specifier.
    None,
    /// `static`.
    Static,
    /// `extern`.
    Extern,
    /// `register`.
    Register,
    /// `auto`.
    Auto,
}

impl CStorageClass {
    /// Return the keyword (including trailing space) for rendering, or
    /// the empty string for `None`.
    pub fn as_prefix(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Static => "static ",
            Self::Extern => "extern ",
            Self::Register => "register ",
            Self::Auto => "auto ",
        }
    }
}

/// A single C declarator (ISO C11 6.7.6 declarator).
///
/// The Phase-7C subset needs only named declarators with optional
/// array / pointer suffixes (represented via [`CType`]). Nested
/// declarators, function-declarators, and abstract declarators are
/// deferred.
#[derive(Debug, Clone)]
pub struct CDeclarator {
    /// Declared identifier.
    pub name: String,
}

impl CDeclarator {
    /// Create a named declarator.
    pub fn named(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// A C declaration (ISO C11 6.7).
#[derive(Debug, Clone)]
pub struct CDeclaration {
    /// Storage-class specifier.
    pub storage: CStorageClass,
    /// Type specifier (possibly compound).
    pub ty: CType,
    /// Declarator (currently a plain named declarator).
    pub declarator: CDeclarator,
    /// Optional initializer (6.7.9).
    pub initializer: Option<CInitializer>,
}

impl CDeclaration {
    /// Construct an uninitialized declaration `<storage> <ty> <name>;`.
    pub fn new(storage: CStorageClass, ty: CType, name: impl Into<String>) -> Self {
        Self {
            storage,
            ty,
            declarator: CDeclarator::named(name),
            initializer: None,
        }
    }

    /// Attach an initializer to this declaration.
    pub fn with_initializer(mut self, init: CInitializer) -> Self {
        self.initializer = Some(init);
        self
    }

    /// Render the declaration at the given indent level. The declaration
    /// is emitted on one line with a terminating newline (except list
    /// initializers, which span multiple lines).
    ///
    /// For array types, the array dimensions are rendered after the
    /// declarator name so the output matches ISO C declarator syntax
    /// (e.g. `int arr[10];`, not `int[10] arr;`).
    pub fn render_into(&self, out: &mut String, indent: usize) {
        for _ in 0..indent {
            out.push('\t');
        }
        out.push_str(self.storage.as_prefix());

        let mut base_ty = &self.ty;
        let mut array_dims: Vec<Option<usize>> = Vec::new();
        while let CType::Array { inner, size } = base_ty {
            array_dims.push(*size);
            base_ty = inner;
        }

        let mut type_text = String::new();
        base_ty.render_into(&mut type_text);
        out.push_str(&type_text);

        if !type_text.ends_with('*') {
            out.push(' ');
        }
        out.push_str(&self.declarator.name);

        for dim in &array_dims {
            match dim {
                Some(n) => out.push_str(&format!("[{}]", n)),
                None => out.push_str("[]"),
            }
        }

        if let Some(init) = &self.initializer {
            out.push_str(" = ");
            init.render_into(out, indent);
        }
        out.push(';');
        out.push('\n');
    }
}
