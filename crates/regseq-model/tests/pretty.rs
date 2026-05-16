//! Integration tests for `PrettyPrinter::render_item` per
//! `pretty-printer.md` §9.9 worked-example specimens (P1a-PP-01).
//!
//! Each specimen pins the exact textual contract for a row in the
//! §5.3 / §5.4 / §5.5 tables; insta snapshots make accidental
//! reformatting a visible event. The §4 / §4.3 Item-header span
//! gating gets a separate set of integration tests at the bottom
//! of this file.

use insta::assert_snapshot;
use regseq_model::{
    DefPath, DefPathData, DefPathSegment, InlineHint, Item, ItemId, ItemKind, Linkage, Mutability,
    PrettyPrintOptions, PrettyPrinter, RegseqModel, SourceFile, SourceFileId, Span, StableCrateId,
};

/// Builder helper: pushes a default-flagged Item carrying `rendered`
/// as the cached `def_path.rendered` and returns its ItemId. The
/// caller mutates `model.items[id]` to opt into specific flags.
fn push_item(model: &mut RegseqModel, kind: ItemKind, rendered_text: &str) -> ItemId {
    let rendered = model.strings.intern(rendered_text);
    let item = Item {
        def_path: DefPath {
            crate_id: model.primary_crate,
            segments: vec![DefPathSegment {
                data: DefPathData::CrateRoot,
                disambiguator: 0,
            }],
            rendered,
        },
        kind,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        linkage: Linkage::Default,
        link_section: None,
        inline_hint: InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
    };
    model.items.push(item)
}

// ------------------------------------------------------------------
// §9.9 render_item specimens (pretty-printer.md:1660-1741).
// ------------------------------------------------------------------

#[test]
fn render_item_specimen_69_bare_function() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = push_item(&mut m, ItemKind::Function, "pl011::init");
    assert_snapshot!(m.pretty().render_item(id), @"fn pl011::init");
}

#[test]
fn render_item_specimen_70_function_full_attribute_stress() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let section = m.strings.intern(".text.boot");
    let id = push_item(&mut m, ItemKind::Function, "bsp::reset");
    {
        let item = &mut m.items[id];
        item.linkage = Linkage::NoMangle;
        item.inline_hint = InlineHint::Always;
        item.link_section = Some(section);
        item.is_unsafe = true;
    }
    assert_snapshot!(m.pretty().render_item(id), @r###"
    #[no_mangle]
    #[inline(always)]
    #[link_section = ".text.boot"]
    unsafe fn bsp::reset
    "###);
}

#[test]
fn render_item_specimen_70b_struct_opaque_handle_eligible() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = push_item(&mut m, ItemKind::Struct, "pl011::Handle");
    m.items[id].opaque_handle_eligible = true;
    assert_snapshot!(m.pretty().render_item(id), @r###"
    #[opaque_handle]
    struct pl011::Handle
    "###);
}

#[test]
fn render_item_specimen_71_static_mut() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = push_item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Mut,
        },
        "CONFIG",
    );
    assert_snapshot!(m.pretty().render_item(id), @"static mut CONFIG");
}

#[test]
fn render_item_specimen_72_const() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = push_item(&mut m, ItemKind::Const, "MAX_BUF_SIZE");
    assert_snapshot!(m.pretty().render_item(id), @"const MAX_BUF_SIZE");
}

#[test]
fn render_item_specimen_73_struct() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = push_item(&mut m, ItemKind::Struct, "pl011::PL011");
    assert_snapshot!(m.pretty().render_item(id), @"struct pl011::PL011");
}

#[test]
fn render_item_specimen_74_enum() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = push_item(&mut m, ItemKind::Enum, "core::option::Option");
    assert_snapshot!(m.pretty().render_item(id), @"enum core::option::Option");
}

#[test]
fn render_item_specimen_75_union_with_lang_flag() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = push_item(&mut m, ItemKind::Union, "core::cell::UnsafeCell");
    m.items[id].is_unsafe_cell = true;
    assert_snapshot!(m.pretty().render_item(id), @r###"
    #[lang = "unsafe_cell"]
    union core::cell::UnsafeCell
    "###);
}

// ------------------------------------------------------------------
// §4 Item-header span gating.
// ------------------------------------------------------------------

#[test]
fn render_item_default_mode_emits_no_span_comment() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = push_item(&mut m, ItemKind::Function, "pl011::init");
    // Even if the span is non-synthesized, default options keep
    // the trailing comment off.
    let path = m.strings.intern("pl011.rs");
    let file_id = m.source_files.push(SourceFile {
        path,
        content: Some("fn init() {}\n".into()),
        line_starts: vec![0],
    });
    m.items[id].span = Span {
        file: file_id,
        lo: 0,
        hi: 12,
    };
    assert_eq!(m.pretty().render_item(id), "fn pl011::init");
}

#[test]
fn render_item_include_spans_emits_aligned_trailing_comment() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = push_item(&mut m, ItemKind::Function, "pl011::init");
    let path = m.strings.intern("pl011.rs");
    let file_id = m.source_files.push(SourceFile {
        path,
        content: Some("fn init() {}\n".into()),
        line_starts: vec![0],
    });
    m.items[id].span = Span {
        file: file_id,
        lo: 0,
        hi: 12,
    };
    let opts = PrettyPrintOptions {
        include_spans: true,
        include_debug_names: false,
    };
    let rendered = PrettyPrinter::with_options(&m, opts).render_item(id);
    // `fn pl011::init` is 14 chars; padded to width 40 then ` // <span>`.
    let header = "fn pl011::init";
    let expected = format!("{header:<40} // pl011.rs:1:1: 1:13");
    assert_eq!(rendered, expected);
}

#[test]
fn render_item_include_spans_suppresses_synthesized_span() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = push_item(&mut m, ItemKind::Function, "pl011::init");
    // span.file == SourceFileId::SYNTHESIZED -> §4.3 suppression
    // applies regardless of include_spans.
    let opts = PrettyPrintOptions {
        include_spans: true,
        include_debug_names: false,
    };
    let rendered = PrettyPrinter::with_options(&m, opts).render_item(id);
    assert_eq!(rendered, "fn pl011::init");
}
