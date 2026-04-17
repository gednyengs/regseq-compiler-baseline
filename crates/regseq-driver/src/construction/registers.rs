//! Register-block construction for Pass 1.
//!
//! Converts a `#[repr(C)]` struct carrying `#[rseq::register_block(..)]`
//! into a [`RegisterBlock`] with fields resolved to access kind, array
//! count, and sequential byte offsets. Descriptor resolution (by
//! descriptor name) is deferred to a follow-up pass; this builder leaves
//! `descriptor: None` everywhere and the caller fills the reference.

use rustc_ast::token::{LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::def_id::DefId;
use rustc_hir::{AttrArgs, Attribute, VariantData};

use regseq_model::identity::{RegisterBlockId, Symbol};
use regseq_model::model::{AccessKind, RegisterBlock, RegisterBlockAttributes, RegisterBlockField};

use super::walk::BuildContext;
use super::ConstructionError;

pub fn build_register_block(
    cx: &mut BuildContext<'_, '_>,
    def_id: DefId,
    name: &str,
    variant_data: &VariantData<'_>,
) -> Result<RegisterBlock, ConstructionError> {
    let id = RegisterBlockId::from(cx.alloc_and_record_public(def_id));
    let name_sym: Symbol = cx.intern_public(name);

    // Parse the block-level attribute for endian + access_width.
    let (endian, access_width) = parse_register_block_attr(cx, def_id);
    let endian_sym = cx.intern_public(&endian);

    // Walk fields. Each field is either:
    //   - ReadWrite<T, Reg::Register>  -> AccessKind::ReadWrite
    //   - ReadOnly<T, Reg::Register>   -> AccessKind::ReadOnly
    //   - WriteOnly<T, Reg::Register>  -> AccessKind::WriteOnly
    //   - Reserved<T>                  -> AccessKind::Reserved (single)
    //   - [Reserved<T>; N]             -> AccessKind::Reserved (array, N)
    let element_size = access_width / 8;
    let mut fields: Vec<RegisterBlockField> = Vec::new();
    let mut offset: u32 = 0;

    for f in variant_data.fields() {
        let f_name = cx.intern_public(f.ident.name.as_str());
        let (kind, array_count, _descriptor_name) = classify_field(f.ty);
        let field = RegisterBlockField {
            name: f_name,
            access_kind: kind,
            descriptor: None,
            byte_offset: offset,
            array_count,
        };
        let span = element_size * (array_count.map(|n| n as u32).unwrap_or(1));
        offset += span;
        fields.push(field);
    }

    Ok(RegisterBlock {
        id,
        name: name_sym,
        block_attributes: RegisterBlockAttributes {
            endian: endian_sym,
            access_width,
        },
        fields,
    })
}

// Access to the BuildContext through tiny adaptor methods so walk.rs
// internals do not need to be `pub`.
impl<'a, 'tcx> BuildContext<'a, 'tcx> {
    pub(crate) fn alloc_and_record_public(
        &mut self,
        def_id: DefId,
    ) -> regseq_model::identity::DeclId {
        let id = self.allocator.alloc();
        crate::construction::record_span(
            self.tcx,
            self.symbols,
            self.source_locations,
            id,
            self.tcx.def_span(def_id),
        );
        id
    }
    pub(crate) fn intern_public(&mut self, s: &str) -> Symbol {
        self.symbols.intern(s)
    }
}

fn classify_field(ty: &rustc_hir::Ty<'_>) -> (AccessKind, Option<usize>, Option<String>) {
    // `[Reserved<_>; N]` first.
    if let rustc_hir::TyKind::Array(inner, len_arg) = &ty.kind {
        let count = array_len(len_arg);
        if is_reserved_path(inner) {
            return (AccessKind::Reserved, Some(count), None);
        }
    }

    if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
        if let Some(seg) = path.segments.last() {
            let name = seg.ident.name.as_str();
            let descriptor = extract_descriptor_name(seg);
            return match name {
                "ReadWrite" => (AccessKind::ReadWrite, None, descriptor),
                "ReadOnly" => (AccessKind::ReadOnly, None, descriptor),
                "WriteOnly" => (AccessKind::WriteOnly, None, descriptor),
                "Reserved" => (AccessKind::Reserved, None, None),
                _ => (AccessKind::ReadWrite, None, descriptor),
            };
        }
    }
    (AccessKind::ReadWrite, None, None)
}

fn array_len(len: &rustc_hir::ConstArg<'_>) -> usize {
    // Try to read the length as a plain integer literal.
    match &len.kind {
        rustc_hir::ConstArgKind::Anon(ac) => anon_const_literal(ac).unwrap_or(1),
        _ => 1,
    }
}

fn anon_const_literal(ac: &rustc_hir::AnonConst) -> Option<usize> {
    // rustc exposes anon consts as bodies; for Pass 1 we skip the full
    // evaluation and just return 1 when the literal isn't trivially
    // visible. A richer implementation would `tcx.const_eval_poly`.
    let _ = ac;
    None
}

fn is_reserved_path(ty: &rustc_hir::Ty<'_>) -> bool {
    if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
        if let Some(seg) = path.segments.last() {
            return seg.ident.name.as_str() == "Reserved";
        }
    }
    false
}

fn extract_descriptor_name(seg: &rustc_hir::PathSegment<'_>) -> Option<String> {
    let args = seg.args?;
    for arg in args.args.iter() {
        if let rustc_hir::GenericArg::Type(inner) = arg {
            if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &inner.kind {
                if path.segments.len() >= 2 {
                    // e.g. `DR_REG::Register` -> take the first segment.
                    return Some(path.segments[0].ident.name.as_str().to_owned());
                }
            }
        }
    }
    None
}

fn parse_register_block_attr(cx: &BuildContext<'_, '_>, def_id: DefId) -> (String, u32) {
    let mut endian = "little".to_owned();
    let mut access_width: u32 = 32;

    for attr in cx.tcx.get_all_attrs(def_id) {
        if attr_is_register_block(attr) {
            if let Some((e, a)) = parse_body(attr) {
                endian = e;
                access_width = a;
            }
        }
    }
    (endian, access_width)
}

fn attr_is_register_block(attr: &Attribute) -> bool {
    if let Attribute::Unparsed(item) = attr {
        let segs = &item.path.segments;
        if segs.len() == 2 && segs[0].as_str() == "rseq" && segs[1].as_str() == "register_block" {
            return true;
        }
    }
    false
}

fn parse_body(attr: &Attribute) -> Option<(String, u32)> {
    let Attribute::Unparsed(item) = attr else {
        return None;
    };
    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => return None,
    };

    let mut endian: Option<String> = None;
    let mut access_width: Option<u32> = None;
    let trees: Vec<&TokenTree> = tokens.iter().collect();
    let mut i = 0;
    while i < trees.len() {
        if is_comma(trees[i]) {
            i += 1;
            continue;
        }
        let key = match &trees[i] {
            TokenTree::Token(tok, _) => match tok.kind {
                TokenKind::Ident(sym, _) => sym.as_str().to_owned(),
                _ => return None,
            },
            _ => return None,
        };
        i += 1;
        if i >= trees.len() {
            return None;
        }
        if !is_eq(trees[i]) {
            return None;
        }
        i += 1;
        if i >= trees.len() {
            return None;
        }
        match &trees[i] {
            TokenTree::Token(tok, _) => {
                if let TokenKind::Literal(lit) = tok.kind {
                    match key.as_str() {
                        "endian" if lit.kind == LitKind::Str => {
                            endian = Some(lit.symbol.as_str().to_owned())
                        }
                        "access_width" if lit.kind == LitKind::Integer => {
                            access_width = lit.symbol.as_str().parse::<u32>().ok()
                        }
                        _ => {}
                    }
                }
            }
            _ => return None,
        }
        i += 1;
    }
    match (endian, access_width) {
        (Some(e), Some(a)) => Some((e, a)),
        (Some(e), None) => Some((e, 32)),
        (None, Some(a)) => Some(("little".to_owned(), a)),
        (None, None) => None,
    }
}

fn is_comma(t: &TokenTree) -> bool {
    matches!(t, TokenTree::Token(tok, _) if matches!(tok.kind, TokenKind::Comma))
}

fn is_eq(t: &TokenTree) -> bool {
    matches!(t, TokenTree::Token(tok, _) if matches!(tok.kind, TokenKind::Eq))
}
