//! Integration tests for stage-1 step-7: the `PrettyPrinter`
//! scaffold.
//!
//! Stage-1 functional methods (`render_span`,
//! `render_def_path_value`) get behavioral tests; later-stage stubs
//! get `#[should_panic]` assertions on their `unimplemented!()`
//! messages so that any change to those messages is a deliberate
//! event picked up by the test suite.

use crate::{
    render_def_path, ConstId, CrateId, CrateInfo, DefPath, DefPathData, DefPathSegment, InstanceId,
    RegseqModel, SourceFile, SourceFileId, Span, StableCrateId, StringId, TyId,
};

// ------------------------------------------------------------------
// Constructor smoke.
// ------------------------------------------------------------------

#[test]
fn pretty_printer_new_constructs_from_model() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    // Both APIs should produce a usable printer without panicking.
    let _ = m.pretty();
    let _ = crate::PrettyPrinter::new(&m);
}

// ------------------------------------------------------------------
// render_span: per the spec sigil, renders `path:line:col: line:col`.
// Synthesized sentinel renders uniformly via the existing
// position-resolution path.
// ------------------------------------------------------------------

#[test]
fn render_span_synthesized_renders_sentinel_path() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let span = Span {
        file: SourceFileId::SYNTHESIZED,
        lo: 0,
        hi: 0,
    };
    assert_eq!(m.pretty().render_span(span), "<synthesized>:1:1: 1:1");
}

#[test]
fn render_span_renders_path_line_col_form() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let path_id = m.strings.intern("foo.rs");
    // Content "abc\ndef\n": line 1 starts at 0; line 2 starts at 4
    // (just past the first '\n'); content length is 8.
    let file_id = m.source_files.push(SourceFile {
        path: path_id,
        content: Some("abc\ndef\n".into()),
        line_starts: vec![0, 4],
    });
    // Span [4, 7) covers "def" -- line 2 col 1 (lo) to line 2
    // col 4 (hi is end-exclusive, the position one past the last
    // byte, i.e., the '\n').
    let span = Span {
        file: file_id,
        lo: 4,
        hi: 7,
    };
    assert_eq!(m.pretty().render_span(span), "foo.rs:2:1: 2:4");
}

#[test]
fn render_span_for_file_without_content_still_renders_positions() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let path_id = m.strings.intern("upstream.rs");
    let file_id = m.source_files.push(SourceFile {
        path: path_id,
        content: None,
        line_starts: vec![0, 10, 20],
    });
    // Position at byte 10 -> line 2 col 1; at byte 12 -> line 2 col 3.
    let span = Span {
        file: file_id,
        lo: 10,
        hi: 12,
    };
    assert_eq!(m.pretty().render_span(span), "upstream.rs:2:1: 2:3");
}

#[test]
fn render_span_path_resolution_uses_interner() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let path_a = m.strings.intern("a.rs");
    let path_b = m.strings.intern("b.rs");
    let file_a = m.source_files.push(SourceFile {
        path: path_a,
        content: None,
        line_starts: vec![0],
    });
    let file_b = m.source_files.push(SourceFile {
        path: path_b,
        content: None,
        line_starts: vec![0],
    });

    let printer = m.pretty();
    let span_a = Span {
        file: file_a,
        lo: 0,
        hi: 0,
    };
    let span_b = Span {
        file: file_b,
        lo: 0,
        hi: 0,
    };
    assert_eq!(printer.render_span(span_a), "a.rs:1:1: 1:1");
    assert_eq!(printer.render_span(span_b), "b.rs:1:1: 1:1");
}

// ------------------------------------------------------------------
// render_def_path_value: thin wrapper over the free helper.
// Functional equivalence test guards against drift if the wrapper
// ever stops forwarding cleanly.
// ------------------------------------------------------------------

#[test]
fn render_def_path_value_renders_canonical_form() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(0xAAAA));
    let foo = m.strings.intern("Foo");

    // Push a second crate to make sure render_def_path_value uses
    // the crate-table prefix from `path.crate_id`, not just the
    // primary crate.
    let upstream_name = m.strings.intern("upstream");
    let upstream_id = m.crates.push(CrateInfo {
        name: upstream_name,
        stable_crate_id: StableCrateId(0xBBBB),
    });

    let path = DefPath {
        crate_id: upstream_id,
        segments: vec![
            DefPathSegment {
                data: DefPathData::CrateRoot,
                disambiguator: 0,
            },
            DefPathSegment {
                data: DefPathData::TypeNs(foo),
                disambiguator: 0,
            },
        ],
        rendered: StringId::ZERO,
    };

    let printer = m.pretty();
    let via_method = printer.render_def_path_value(&path);
    let via_helper = render_def_path(&path, &m.crates, &m.strings);
    assert_eq!(via_method, via_helper);
    assert_eq!(via_method, "upstream::Foo");
}

#[test]
fn render_def_path_value_handles_primary_crate_paths() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(0xAAAA));
    let path = DefPath {
        crate_id: CrateId::ZERO,
        segments: vec![DefPathSegment {
            data: DefPathData::CrateRoot,
            disambiguator: 0,
        }],
        rendered: StringId::ZERO,
    };
    assert_eq!(m.pretty().render_def_path_value(&path), "pl011-spec");
}

// ------------------------------------------------------------------
// Stage-N stubs: each must panic with an informative message that
// names the stage filling the method. Tests pin the exact wording
// fragment so any change to the stub messages is a deliberate
// event.
// ------------------------------------------------------------------

#[test]
#[should_panic(expected = "stage 12+")]
fn render_top_level_is_unimplemented_at_stage_one() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let _ = m.pretty().render();
}

#[test]
#[should_panic(expected = "stage 13 fills render_ty")]
fn render_ty_is_unimplemented_at_stage_one() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let _ = m.pretty().render_ty(TyId::ZERO);
}

#[test]
#[should_panic(expected = "stage 14 fills render_const")]
fn render_const_is_unimplemented_at_stage_one() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let _ = m.pretty().render_const(ConstId::ZERO);
}

#[test]
#[should_panic(expected = "stage 14 fills render_instance")]
fn render_instance_is_unimplemented_at_stage_one() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let _ = m.pretty().render_instance(InstanceId::ZERO);
}

// ------------------------------------------------------------------
// render_item_attribute_block: §5.6 shared Stream-1 helper.
//
// These tests exercise the private dispatch logic of the helper
// (per the §5.3 per-flag table, §5.2 default-suppression, and the
// §5.5 trailing-newline-when-non-empty rule). Access to the private
// method works because `mod tests` is a child of the `pretty`
// module.
// ------------------------------------------------------------------

use crate::item::{InlineHint, Item, ItemKind, Linkage};
use crate::ty::Mutability;
use crate::ItemId;

fn unit_item(model: &mut RegseqModel, kind: ItemKind, rendered: StringId) -> ItemId {
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

#[test]
fn render_item_attribute_block_is_empty_for_all_defaults() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011-spec");
    let id = unit_item(&mut m, ItemKind::Function, rendered);
    assert_eq!(m.pretty().render_item_attribute_block(id), "");
}

#[test]
fn render_item_attribute_block_walks_stream_one_rows_in_table_order() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011-spec");
    let section = m.strings.intern(".init_array");
    let id = unit_item(&mut m, ItemKind::Struct, rendered);
    {
        let item = &mut m.items[id];
        item.linkage = Linkage::NoMangle;
        item.inline_hint = InlineHint::Always;
        item.is_unsafe_cell = true;
        item.link_section = Some(section);
        item.opaque_handle_eligible = true;
    }
    let expected = "#[no_mangle]\n\
                    #[inline(always)]\n\
                    #[lang = \"unsafe_cell\"]\n\
                    #[link_section = \".init_array\"]\n\
                    #[opaque_handle]\n";
    assert_eq!(m.pretty().render_item_attribute_block(id), expected);
}

#[test]
fn render_item_attribute_block_does_not_surface_is_unsafe() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011-spec");
    let id = unit_item(&mut m, ItemKind::Function, rendered);
    m.items[id].is_unsafe = true;
    // `is_unsafe` is the kind-keyword prefix's responsibility, not
    // the helper's: the helper produces no attribute row for it.
    assert_eq!(m.pretty().render_item_attribute_block(id), "");
}

#[test]
fn render_item_attribute_block_phantom_data_row_in_isolation() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011-spec");
    let id = unit_item(&mut m, ItemKind::Struct, rendered);
    m.items[id].is_phantom_data = true;
    assert_eq!(
        m.pretty().render_item_attribute_block(id),
        "#[lang = \"phantom_data\"]\n"
    );
}

#[test]
fn render_item_attribute_block_unsafe_cell_row_in_isolation() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011-spec");
    let id = unit_item(&mut m, ItemKind::Union, rendered);
    m.items[id].is_unsafe_cell = true;
    assert_eq!(
        m.pretty().render_item_attribute_block(id),
        "#[lang = \"unsafe_cell\"]\n"
    );
}

#[test]
fn render_item_attribute_block_manually_drop_row_in_isolation() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011-spec");
    let id = unit_item(&mut m, ItemKind::Struct, rendered);
    m.items[id].is_manually_drop = true;
    assert_eq!(
        m.pretty().render_item_attribute_block(id),
        "#[lang = \"manually_drop\"]\n"
    );
}

#[test]
fn render_item_attribute_block_trailing_newline_when_non_empty() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011-spec");
    let id = unit_item(&mut m, ItemKind::Function, rendered);
    m.items[id].linkage = Linkage::NoMangle;
    let out = m.pretty().render_item_attribute_block(id);
    assert!(
        out.ends_with('\n'),
        "expected helper to end in a trailing newline when non-empty, got {out:?}"
    );
}

#[test]
fn render_item_attribute_block_static_kind_does_not_inject_unsafe_attribute() {
    // `unsafe static` is non-syntactic; the helper has no row for
    // is_unsafe regardless of kind, so a Static with is_unsafe set
    // is still attribute-empty.
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011-spec");
    let id = unit_item(
        &mut m,
        ItemKind::Static {
            mutability: Mutability::Mut,
        },
        rendered,
    );
    m.items[id].is_unsafe = true;
    assert_eq!(m.pretty().render_item_attribute_block(id), "");
}

#[test]
fn render_item_attribute_block_linkage_foreign_emits_foreign() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011-spec");
    let id = unit_item(&mut m, ItemKind::Function, rendered);
    m.items[id].linkage = Linkage::Foreign;
    assert_eq!(m.pretty().render_item_attribute_block(id), "#[foreign]\n");
}

#[test]
fn render_item_attribute_block_linkage_weak_emits_key_value_form() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011-spec");
    let id = unit_item(&mut m, ItemKind::Function, rendered);
    m.items[id].linkage = Linkage::Weak;
    assert_eq!(
        m.pretty().render_item_attribute_block(id),
        "#[linkage = \"weak\"]\n"
    );
}

#[test]
fn render_item_attribute_block_inline_hint_each_form() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011-spec");

    let hint = unit_item(&mut m, ItemKind::Function, rendered);
    m.items[hint].inline_hint = InlineHint::Hint;
    assert_eq!(m.pretty().render_item_attribute_block(hint), "#[inline]\n");

    let never = unit_item(&mut m, ItemKind::Function, rendered);
    m.items[never].inline_hint = InlineHint::Never;
    assert_eq!(
        m.pretty().render_item_attribute_block(never),
        "#[inline(never)]\n"
    );
}
