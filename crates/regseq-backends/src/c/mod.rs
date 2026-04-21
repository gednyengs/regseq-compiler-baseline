//! Backend-neutral C AST (ISO C11 6.7-6.9 subset).
//!
//! Houses the typed representation of C translation units used by every
//! C-targeting backend (Linux today, Zephyr tomorrow). Linux-specific
//! policy such as `SPDX-License-Identifier: GPL-2.0` banners lives in
//! [`crate::linux::conventions`], not here.
//!
//! # Scope
//!
//! The AST covers the subset that Phase-7C CFG emission requires:
//!
//! - Translation-unit scaffolding: includes, defines, include guards,
//!   struct definitions, static / const globals with designated
//!   initializers, function definitions (legacy string-body form and
//!   structured form backed by [`statements::CStmt::Compound`]).
//! - Statements ([`statements`]): compound, declaration, expression,
//!   `if` / `if-else`, `while`, `return`, `goto`, labeled, empty, raw.
//! - Expressions ([`expressions`]): integer / string literals,
//!   identifiers, field projection (`.` and `->`), array index,
//!   function call, unary / binary operators, cast, assignment (plain
//!   and compound), ternary, paren, raw.
//! - Declarators, types, storage classes ([`declarators`]).
//! - Initializers and designators ([`initializers`]).
//!
//! # Deliberately deferred
//!
//! Per migration.md Q8 ("Phase-7C urgent subset"), the following C
//! constructs are **intentionally absent** until a live driver needs
//! them: `for` / `do while` iteration, `break` / `continue`, `switch`
//! / `case`, `typedef`, `enum`, `union`, preprocessor conditionals
//! (`#if` / `#else` / `#endif`), and parameterized function-like
//! macros. Their absence is a scope choice, not an oversight.

use std::fmt::Write;

pub mod declarators;
pub mod expressions;
pub mod initializers;
pub mod statements;

pub use declarators::{CDeclaration, CDeclarator, CStorageClass, CType};
pub use expressions::{CBinaryOp, CExpr, CIntSuffix, CUnaryOp};
pub use initializers::{CDesignator, CInitItem, CInitializer};
pub use statements::{CBlockItem, CStmt};

// ---------------------------------------------------------------------------
// Translation unit
// ---------------------------------------------------------------------------

/// A complete C translation unit (one `.c` or `.h` file).
#[derive(Debug, Clone)]
pub struct CTranslationUnit {
    items: Vec<CTopLevelItem>,
}

impl CTranslationUnit {
    /// Create an empty translation unit.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Append a top-level item.
    pub fn push(&mut self, item: CTopLevelItem) {
        self.items.push(item);
    }

    /// Render the entire translation unit to a C source string. Items
    /// render in insertion order; each item emits its own trailing
    /// newline, and an additional blank-line separator is inserted
    /// between consecutive items (excluding the last).
    pub fn render(&self) -> String {
        let mut out = String::new();
        for (i, item) in self.items.iter().enumerate() {
            item.render_into(&mut out);
            if i + 1 < self.items.len() {
                out.push('\n');
            }
        }
        out
    }
}

impl Default for CTranslationUnit {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Top-level items
// ---------------------------------------------------------------------------

/// A top-level item in a C translation unit.
#[derive(Debug, Clone)]
pub enum CTopLevelItem {
    /// Multi-line block comment (`/* ... */`).
    BlockComment(Vec<String>),
    /// Single-line comment rendered as `/* ... */`.
    LineComment(String),
    /// Blank line separator.
    BlankLine,
    /// `#include <path>` (system) or `#include "path"` (local).
    Include {
        /// Header path.
        path: String,
        /// True for `<...>`, false for `"..."`.
        system: bool,
    },
    /// `#define NAME` or `#define NAME VALUE`.
    Define {
        /// Macro name.
        name: String,
        /// Optional macro value.
        value: Option<String>,
    },
    /// Include guard opening: `#ifndef GUARD` + `#define GUARD`.
    IncludeGuardOpen(String),
    /// Include guard closing: `#endif /* GUARD */`.
    IncludeGuardClose(String),
    /// Struct definition with named fields.
    StructDef {
        /// Struct tag name.
        name: String,
        /// Struct fields.
        fields: Vec<CStructField>,
    },
    /// Static / const variable definition with a designated-initializer
    /// body. The initializer body is a vector of already-rendered lines;
    /// tasks 9 / 10 migrate call sites to construct [`CInitializer`]
    /// values instead.
    StaticDef {
        /// Qualifiers (e.g. `"static const"`).
        qualifiers: String,
        /// C type expression (e.g. `"struct amba_id"`).
        type_name: String,
        /// Variable name (may include `[]` for arrays).
        name: String,
        /// Initializer lines (rendered inside `{ ... }`).
        initializer: Vec<String>,
    },
    /// Function definition with a string-formatted body (legacy form).
    FuncDef {
        /// Full function signature line (e.g. `"static int foo(int x)"`).
        signature: String,
        /// Body lines (indented by the renderer).
        body: Vec<String>,
        /// Optional provenance comment above the function.
        comment: Option<String>,
    },
    /// Function definition with a structured [`CStmt::Compound`] body.
    ///
    /// Added for Phase-7C CFG emission; coexists with [`Self::FuncDef`]
    /// so tasks 9 / 10 can migrate call sites incrementally.
    FuncDefStructured {
        /// Full function signature line.
        signature: String,
        /// Structured body. Expected to be [`CStmt::Compound`]; other
        /// variants are still accepted and wrapped in a compound shell
        /// by the renderer.
        body: CStmt,
        /// Optional provenance comment above the function.
        comment: Option<String>,
    },
    /// Preprocessor macro invocation on its own line.
    MacroCall {
        /// Macro name (e.g. `"MODULE_LICENSE"`).
        name: String,
        /// Argument string (e.g. `"\"GPL\""`).
        args: String,
    },
    /// Raw C text (escape hatch for constructs the AST does not model).
    Raw(String),
}

/// A field inside a struct definition.
#[derive(Debug, Clone)]
pub struct CStructField {
    /// C type expression.
    pub type_name: String,
    /// Field name.
    pub field_name: String,
    /// Optional inline doc comment.
    pub doc: Option<String>,
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

impl CTopLevelItem {
    /// Render the top-level item (including its trailing newline).
    pub fn render_into(&self, out: &mut String) {
        match self {
            Self::BlockComment(lines) => {
                out.push_str("/*\n");
                for line in lines {
                    if line.is_empty() {
                        out.push_str(" *\n");
                    } else {
                        let _ = writeln!(out, " * {line}");
                    }
                }
                out.push_str(" */\n");
            }
            Self::LineComment(text) => {
                let _ = writeln!(out, "/* {text} */");
            }
            Self::BlankLine => {
                // The newline after each item in render() handles spacing.
            }
            Self::Include { path, system } => {
                if *system {
                    let _ = writeln!(out, "#include <{path}>");
                } else {
                    let _ = writeln!(out, "#include \"{path}\"");
                }
            }
            Self::Define { name, value } => {
                if let Some(v) = value {
                    let _ = writeln!(out, "#define {name} {v}");
                } else {
                    let _ = writeln!(out, "#define {name}");
                }
            }
            Self::IncludeGuardOpen(guard) => {
                let _ = writeln!(out, "#ifndef {guard}");
                let _ = writeln!(out, "#define {guard}");
            }
            Self::IncludeGuardClose(guard) => {
                let _ = writeln!(out, "#endif /* {guard} */");
            }
            Self::StructDef { name, fields } => {
                let _ = writeln!(out, "struct {name} {{");
                for f in fields {
                    if let Some(ref doc) = f.doc {
                        let _ = writeln!(out, "\t/* {doc} */");
                    }
                    let _ = writeln!(out, "\t{} {};", f.type_name, f.field_name);
                }
                let _ = writeln!(out, "}};");
            }
            Self::StaticDef {
                qualifiers,
                type_name,
                name,
                initializer,
            } => {
                let _ = writeln!(out, "{qualifiers} {type_name} {name} = {{");
                for line in initializer {
                    let _ = writeln!(out, "\t{line}");
                }
                let _ = writeln!(out, "}};");
            }
            Self::FuncDef {
                signature,
                body,
                comment,
            } => {
                if let Some(c) = comment {
                    let _ = writeln!(out, "/* {c} */");
                }
                let _ = writeln!(out, "{signature}\n{{");
                for line in body {
                    if line.is_empty() {
                        out.push('\n');
                    } else {
                        let _ = writeln!(out, "\t{line}");
                    }
                }
                out.push_str("}\n");
            }
            Self::FuncDefStructured {
                signature,
                body,
                comment,
            } => {
                if let Some(c) = comment {
                    let _ = writeln!(out, "/* {c} */");
                }
                out.push_str(signature);
                out.push('\n');
                match body {
                    CStmt::Compound { items } => {
                        out.push_str("{\n");
                        for item in items {
                            item.render_into(out, 1);
                        }
                        out.push_str("}\n");
                    }
                    other => {
                        // Non-compound bodies: wrap in synthetic braces
                        // so the output remains a valid function body.
                        out.push_str("{\n");
                        other.render_into(out, 1);
                        out.push_str("}\n");
                    }
                }
            }
            Self::MacroCall { name, args } => {
                let _ = writeln!(out, "{name}({args});");
            }
            Self::Raw(text) => {
                out.push_str(text);
                if !text.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Builder helpers (backend-neutral)
// ---------------------------------------------------------------------------

/// Create an include guard name from a filename.
///
/// Backend-neutral: Linux / Zephyr / other C backends all use the same
/// uppercase-filename convention. Linux-specific file banners live in
/// [`crate::linux::conventions`].
pub fn include_guard_name(filename: &str) -> String {
    filename.to_uppercase().replace(['.', '-', '/'], "_")
}
