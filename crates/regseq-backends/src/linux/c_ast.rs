//! Lightweight C AST for structured Linux backend emission.
//!
//! Provides a small typed representation of C translation units that
//! the Linux emitter uses to produce readable, deterministic output.
//! This avoids opaque string concatenation while keeping the AST
//! focused on what the backend actually needs.

use std::fmt::Write;

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

    /// Render the entire translation unit to a C source string.
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
    /// Single-line comment (`// ...`).
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
    /// Static/const variable definition with a designated-initializer body.
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
    /// Function definition.
    FuncDef {
        /// Full function signature line (e.g. `"static int foo(int x)"`).
        signature: String,
        /// Body lines (indented by the renderer).
        body: Vec<String>,
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
    fn render_into(&self, out: &mut String) {
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
// Builder helpers
// ---------------------------------------------------------------------------

/// Create a standard Regseq generated-file banner comment.
pub fn generated_file_banner(filename: &str, driver_name: &str) -> CTopLevelItem {
    CTopLevelItem::BlockComment(vec![
        format!("SPDX-License-Identifier: GPL-2.0"),
        String::new(),
        format!("{filename} - Regseq-generated Linux driver artifact"),
        String::new(),
        format!("Generated by Regseq compiler. Do not edit."),
        format!("Driver: {driver_name}"),
    ])
}

/// Create an include guard name from a filename.
pub fn include_guard_name(filename: &str) -> String {
    filename.to_uppercase().replace(['.', '-', '/'], "_")
}
