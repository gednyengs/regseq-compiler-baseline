//! Integration tests for stage-1 step-7: the `PrettyPrinter`
//! scaffold.
//!
//! Stage-1 functional methods (`render_span`,
//! `render_def_path_value`) get behavioral tests; later-stage stubs
//! get `#[should_panic]` assertions on their `unimplemented!()`
//! messages so that any change to those messages is a deliberate
//! event picked up by the test suite.

use crate::{
    render_def_path, CrateId, CrateInfo, DefPath, DefPathData, DefPathSegment, InstanceId,
    RegseqModel, SourceFile, SourceFileId, Span, StableCrateId, StringId,
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

// ------------------------------------------------------------------
// Unit tests for `render_ty` per-arm dispatch
// (pretty-printer.md §6.1 - §6.6) and `render_generic_args`
// (§6.7). Per the spec's Test Plan: cover per-arm dispatch shape;
// the §9.1 worked-example specimens live in
// `tests/pretty.rs` as insta snapshots.
// ------------------------------------------------------------------

use crate::args_key::GenericArg;
use crate::body::Body;
use crate::fn_sig::FnSig;
use crate::instance::{FunctionInstanceData, FunctionInstanceFlavor, Instance, InstanceKind};
use crate::ty::{AutoTrait, FloatKind, FnAbi, IntKind, Provenance, Safety, Ty, UintKind};

#[test]
fn render_ty_primitives_match_section_6_1() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let cases: Vec<(Ty, &'static str)> = vec![
        (Ty::Bool, "bool"),
        (Ty::Char, "char"),
        (Ty::Int(IntKind::I8), "i8"),
        (Ty::Int(IntKind::I16), "i16"),
        (Ty::Int(IntKind::I32), "i32"),
        (Ty::Int(IntKind::I64), "i64"),
        (Ty::Int(IntKind::I128), "i128"),
        (Ty::Int(IntKind::Isize), "isize"),
        (Ty::Uint(UintKind::U8), "u8"),
        (Ty::Uint(UintKind::U16), "u16"),
        (Ty::Uint(UintKind::U32), "u32"),
        (Ty::Uint(UintKind::U64), "u64"),
        (Ty::Uint(UintKind::U128), "u128"),
        (Ty::Uint(UintKind::Usize), "usize"),
        (Ty::Float(FloatKind::F16), "f16"),
        (Ty::Float(FloatKind::F32), "f32"),
        (Ty::Float(FloatKind::F64), "f64"),
        (Ty::Float(FloatKind::F128), "f128"),
        (Ty::Str, "str"),
        (Ty::Never, "!"),
    ];
    for (ty, expected) in cases {
        let id = m.types.intern(ty);
        assert_eq!(m.pretty().render_ty(id), expected);
    }
}

#[test]
fn render_ty_array_uses_decimal_length() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let elem = m.types.intern(Ty::Uint(UintKind::U8));
    let array = m.types.intern(Ty::Array(elem, 32));
    assert_eq!(m.pretty().render_ty(array), "[u8; 32]");
}

#[test]
fn render_ty_slice_recurses_into_element() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let elem = m.types.intern(Ty::Uint(UintKind::U8));
    let slice = m.types.intern(Ty::Slice(elem));
    assert_eq!(m.pretty().render_ty(slice), "[u8]");
}

#[test]
fn render_ty_ref_and_ptr_dispatch_all_four_combinations() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));

    let ref_shared = m.types.intern(Ty::Ref(i32_ty, Provenance::SharedBorrow));
    let ref_unique = m.types.intern(Ty::Ref(i32_ty, Provenance::UniqueBorrow));
    let ptr_const = m.types.intern(Ty::Ptr(u8_ty, Mutability::Not));
    let ptr_mut = m.types.intern(Ty::Ptr(u8_ty, Mutability::Mut));

    let p = m.pretty();
    assert_eq!(p.render_ty(ref_shared), "&i32");
    assert_eq!(p.render_ty(ref_unique), "&mut i32");
    assert_eq!(p.render_ty(ptr_const), "*const u8");
    assert_eq!(p.render_ty(ptr_mut), "*mut u8");
}

#[test]
fn render_ty_tuple_arity_zero_one_and_two() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let bool_ty = m.types.intern(Ty::Bool);
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: crate::UNIT_LAYOUT,
    });
    let arity_one = m.types.intern(Ty::Tuple {
        fields: vec![i32_ty],
        layout: crate::UNIT_LAYOUT,
    });
    let arity_two = m.types.intern(Ty::Tuple {
        fields: vec![i32_ty, bool_ty],
        layout: crate::UNIT_LAYOUT,
    });
    let p = m.pretty();
    assert_eq!(p.render_ty(unit), "()");
    assert_eq!(p.render_ty(arity_one), "(i32,)");
    assert_eq!(p.render_ty(arity_two), "(i32, bool)");
}

#[test]
fn render_ty_struct_enum_union_compose_path_then_args() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let pl011_name = m.strings.intern("pl011::PL011");
    let option_name = m.strings.intern("core::option::Option");
    let my_union_name = m.strings.intern("pl011::regs::MyUnion");
    let pl011_id = unit_item(&mut m, ItemKind::Struct, pl011_name);
    let option_id = unit_item(&mut m, ItemKind::Enum, option_name);
    let my_union_id = unit_item(&mut m, ItemKind::Union, my_union_name);
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));

    let pl011 = m.types.intern(Ty::Struct {
        item: pl011_id,
        args: vec![],
        layout: crate::UNIT_LAYOUT,
    });
    let option_u32 = m.types.intern(Ty::Enum {
        item: option_id,
        args: vec![GenericArg::Type(u32_ty)],
        layout: crate::UNIT_LAYOUT,
    });
    let my_union = m.types.intern(Ty::Union {
        item: my_union_id,
        args: vec![],
        layout: crate::UNIT_LAYOUT,
    });

    let p = m.pretty();
    assert_eq!(p.render_ty(pl011), "pl011::PL011");
    assert_eq!(p.render_ty(option_u32), "core::option::Option<u32>");
    assert_eq!(p.render_ty(my_union), "pl011::regs::MyUnion");
}

#[test]
fn render_ty_closure_with_generic_args() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let closure_name = m.strings.intern("pl011::init::{closure#0}");
    let item_id = unit_item(&mut m, ItemKind::Function, closure_name);
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let closure = m
        .types
        .intern(Ty::Closure(item_id, vec![GenericArg::Type(i32_ty)]));
    assert_eq!(
        m.pretty().render_ty(closure),
        "pl011::init::{closure#0}<i32>"
    );
}

#[test]
fn render_ty_fn_def_renders_curly_brace_form_with_instance_lookup() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let pl011_name = m.strings.intern("pl011::PL011");
    let init_name = m.strings.intern("pl011::init");
    let pl011_id = unit_item(&mut m, ItemKind::Struct, pl011_name);
    let init_id = unit_item(&mut m, ItemKind::Function, init_name);

    let pl011_ty = m.types.intern(Ty::Struct {
        item: pl011_id,
        args: vec![],
        layout: crate::UNIT_LAYOUT,
    });
    let ptr_ty = m.types.intern(Ty::Ptr(pl011_ty, Mutability::Mut));
    let bool_ty = m.types.intern(Ty::Bool);
    let sig = m.sigs.intern(FnSig {
        inputs: vec![ptr_ty],
        output: bool_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });

    let emission_name = m.strings.intern("pl011_init");
    m.instances.push(Instance {
        item: init_id,
        generic_args: vec![],
        emission_name,
        span: Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        },
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig,
            body: Some(Body::default()),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });

    let fn_def = m.types.intern(Ty::FnDef(init_id, vec![]));
    assert_eq!(
        m.pretty().render_ty(fn_def),
        "fn(*mut pl011::PL011) -> bool {pl011::init}"
    );
}

#[test]
fn render_ty_fn_ptr_prefix_combinations() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let ptr_u8 = m.types.intern(Ty::Ptr(u8_ty, Mutability::Mut));

    let safe_rust = m.sigs.intern(FnSig {
        inputs: vec![i32_ty],
        output: u32_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let safe_c = m.sigs.intern(FnSig {
        inputs: vec![i32_ty],
        output: u32_ty,
        abi: FnAbi::C,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let unsafe_rust = m.sigs.intern(FnSig {
        inputs: vec![i32_ty],
        output: u32_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Unsafe,
    });
    let unsafe_c = m.sigs.intern(FnSig {
        inputs: vec![ptr_u8],
        output: i32_ty,
        abi: FnAbi::C,
        c_variadic: false,
        safety: Safety::Unsafe,
    });

    let p_safe_rust = m.types.intern(Ty::FnPtr(safe_rust));
    let p_safe_c = m.types.intern(Ty::FnPtr(safe_c));
    let p_unsafe_rust = m.types.intern(Ty::FnPtr(unsafe_rust));
    let p_unsafe_c = m.types.intern(Ty::FnPtr(unsafe_c));

    let p = m.pretty();
    assert_eq!(p.render_ty(p_safe_rust), "fn(i32) -> u32");
    assert_eq!(p.render_ty(p_safe_c), "extern \"C\" fn(i32) -> u32");
    assert_eq!(p.render_ty(p_unsafe_rust), "unsafe fn(i32) -> u32");
    assert_eq!(
        p.render_ty(p_unsafe_c),
        "unsafe extern \"C\" fn(*mut u8) -> i32"
    );
}

#[test]
fn render_ty_dynamic_with_empty_and_sorted_auto_traits() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let vtable_path = m
        .strings
        .intern("<regseq_synth>::vtable::Reader::for::<std::fs::File>");
    let vtable_item = unit_item(&mut m, ItemKind::Struct, vtable_path);
    let vtable_ty = m.types.intern(Ty::Struct {
        item: vtable_item,
        args: vec![],
        layout: crate::UNIT_LAYOUT,
    });

    let with_autos = m.types.intern(Ty::Dynamic {
        auto_traits: vec![AutoTrait::Send, AutoTrait::Sync],
        vtable_ty,
    });
    let without_autos = m.types.intern(Ty::Dynamic {
        auto_traits: vec![],
        vtable_ty,
    });

    let p = m.pretty();
    assert_eq!(
        p.render_ty(with_autos),
        "dyn <regseq_synth>::vtable::Reader::for::<std::fs::File> + Send + Sync"
    );
    assert_eq!(
        p.render_ty(without_autos),
        "dyn <regseq_synth>::vtable::Reader::for::<std::fs::File>"
    );
}

#[test]
fn render_ty_dynamic_defensively_sorts_unsorted_auto_traits() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let vtable_path = m.strings.intern("<regseq_synth>::vtable::Foo::for::<Bar>");
    let vtable_item = unit_item(&mut m, ItemKind::Struct, vtable_path);
    let vtable_ty = m.types.intern(Ty::Struct {
        item: vtable_item,
        args: vec![],
        layout: crate::UNIT_LAYOUT,
    });

    // Construct an input with deliberately reversed-order auto traits.
    // The renderer must emit the §10.1 sorted order (Send < Sync < Unpin)
    // regardless, matching the canonical-encoding defensive sort.
    let id = m.types.intern(Ty::Dynamic {
        auto_traits: vec![AutoTrait::Unpin, AutoTrait::Send],
        vtable_ty,
    });
    assert_eq!(
        m.pretty().render_ty(id),
        "dyn <regseq_synth>::vtable::Foo::for::<Bar> + Send + Unpin"
    );
}

#[test]
fn render_generic_args_empty_returns_empty_string() {
    let m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = crate::ItemId::ZERO;
    assert_eq!(m.pretty().render_generic_args(&vec![], item), "");
}

#[test]
fn render_generic_args_renders_type_args_with_angle_bracket_wrap() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let bool_ty = m.types.intern(Ty::Bool);
    // ItemId arg is irrelevant for Type-only args (it only matters
    // for the still-`todo!()` Const arm).
    let item = crate::ItemId::ZERO;
    let args = vec![GenericArg::Type(i32_ty), GenericArg::Type(bool_ty)];
    assert_eq!(m.pretty().render_generic_args(&args, item), "<i32, bool>");
}

// ==================================================================
// Body-level renderers (pretty-printer.md §7.1 - §7.10).
// ==================================================================

use crate::args_key::GenericArgs;
use crate::basic_block::BasicBlock;
use crate::layout::{EnumShape, FieldLayout, Layout, LayoutKind, VariantLayout};
use crate::operand::Operand;
use crate::place::{Place, PlaceElem};
use crate::rvalue::{AggregateKind, BinOp, CastKind, PointerCoercion, Rvalue, UnOp};
use crate::statement::{CopyNonOverlapping, NonDivergingIntrinsic, Statement, StatementKind};
use crate::terminator::{AssertKind, CallCallee, SwitchTargets, Terminator, TerminatorKind};
use crate::{
    BlockId, FieldIdx, LocalDecl, LocalId, PrettyPrintOptions, PrettyPrinter, TyId, VariantIdx,
};

fn dummy_local(ty: TyId) -> LocalDecl {
    LocalDecl {
        ty,
        mutability: Mutability::Mut,
        span: Span::dummy(),
        debug_name: None,
    }
}

fn push_locals(body: &mut Body, count: usize, ty: TyId) {
    for _ in 0..count {
        body.push_local(dummy_local(ty));
    }
}

fn unit_ty(m: &mut RegseqModel) -> TyId {
    m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: crate::UNIT_LAYOUT,
    })
}

// ----- render_place ------------------------------------------------

#[test]
fn render_place_bare_local_renders_underscore_n() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, 4, u32_ty);
    let place = Place::from(LocalId::from_u32(3));
    assert_eq!(m.pretty().render_place(&place, &body), "_3");
}

#[test]
fn render_place_deref_wraps_in_parens_star() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let ptr_ty = m.types.intern(Ty::Ptr(u32_ty, Mutability::Mut));
    let mut body = Body::default();
    push_locals(&mut body, 1, u32_ty);
    body.push_local(dummy_local(ptr_ty));
    let place = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([PlaceElem::Deref]),
    };
    assert_eq!(m.pretty().render_place(&place, &body), "(*_1)");
}

#[test]
fn render_place_index_wraps_in_brackets_with_local_sigil() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let array_ty = m.types.intern(Ty::Array(u32_ty, 8));
    let mut body = Body::default();
    push_locals(&mut body, 4, u32_ty);
    body.push_local(dummy_local(array_ty));
    let place = Place {
        local: LocalId::from_u32(4),
        projection: Box::new([PlaceElem::Index(LocalId::from_u32(2))]),
    };
    assert_eq!(m.pretty().render_place(&place, &body), "_4[_2]");
}

#[test]
fn render_place_constant_index_from_start_renders_offset_min_length() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let array_ty = m.types.intern(Ty::Array(u32_ty, 8));
    let mut body = Body::default();
    body.push_local(dummy_local(array_ty));
    let place = Place {
        local: LocalId::from_u32(0),
        projection: Box::new([PlaceElem::ConstantIndex {
            offset: 1,
            min_length: 4,
            from_end: false,
        }]),
    };
    assert_eq!(m.pretty().render_place(&place, &body), "_0[1 of 4]");
}

#[test]
fn render_place_constant_index_from_end_renders_negative_offset() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let slice_ty = m.types.intern(Ty::Slice(u32_ty));
    let mut body = Body::default();
    body.push_local(dummy_local(slice_ty));
    let place = Place {
        local: LocalId::from_u32(0),
        projection: Box::new([PlaceElem::ConstantIndex {
            offset: 1,
            min_length: 4,
            from_end: true,
        }]),
    };
    assert_eq!(m.pretty().render_place(&place, &body), "_0[-1 of 4]");
}

#[test]
fn render_place_subslice_from_start_renders_from_to_range() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let array_ty = m.types.intern(Ty::Array(u32_ty, 8));
    let mut body = Body::default();
    body.push_local(dummy_local(array_ty));
    let place = Place {
        local: LocalId::from_u32(0),
        projection: Box::new([PlaceElem::Subslice {
            from: 0,
            to: 3,
            from_end: false,
        }]),
    };
    assert_eq!(m.pretty().render_place(&place, &body), "_0[0..3]");
}

#[test]
fn render_place_subslice_from_end_renders_minus_to() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let slice_ty = m.types.intern(Ty::Slice(u32_ty));
    let mut body = Body::default();
    body.push_local(dummy_local(slice_ty));
    let place = Place {
        local: LocalId::from_u32(0),
        projection: Box::new([PlaceElem::Subslice {
            from: 1,
            to: 0,
            from_end: true,
        }]),
    };
    assert_eq!(m.pretty().render_place(&place, &body), "_0[1..-0]");
}

#[test]
fn render_place_field_renders_named_field_and_post_narrow_ty() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mask_name = m.strings.intern("mask");
    let pl011_name = m.strings.intern("PL011");
    let pl011 = unit_item(&mut m, ItemKind::Struct, pl011_name);
    let pl011_ty = m.types.intern(Ty::Struct {
        item: pl011,
        args: vec![],
        layout: Layout {
            size: 4,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![FieldLayout {
                    name: mask_name,
                    ty: u32_ty,
                    offset: 0,
                }],
            },
        },
    });
    let mut body = Body::default();
    body.push_local(dummy_local(pl011_ty));
    let place = Place {
        local: LocalId::from_u32(0),
        projection: Box::new([PlaceElem::Field(FieldIdx::from_u32(0), u32_ty)]),
    };
    assert_eq!(m.pretty().render_place(&place, &body), "(_0.mask: u32)");
}

#[test]
fn render_place_downcast_renders_variant_name() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let some_name = m.strings.intern("Some");
    let none_name = m.strings.intern("None");
    let option_name = m.strings.intern("core::option::Option");
    let option_item = unit_item(&mut m, ItemKind::Enum, option_name);
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let option_ty = m.types.intern(Ty::Enum {
        item: option_item,
        args: vec![GenericArg::Type(u32_ty)],
        layout: Layout {
            size: 8,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Enum {
                shape: EnumShape::Multiple {
                    tag: crate::layout::TagScalar {
                        offset: 0,
                        scalar_ty: u32_ty,
                        valid_range: crate::layout::WrappingRange { start: 0, end: 1 },
                    },
                    encoding: crate::layout::TagEncoding::Direct,
                    variants: vec![
                        VariantLayout {
                            name: none_name,
                            fields: vec![],
                            uninhabited: false,
                            discriminant: 0,
                            tag_value: Some(0),
                        },
                        VariantLayout {
                            name: some_name,
                            fields: vec![],
                            uninhabited: false,
                            discriminant: 1,
                            tag_value: Some(1),
                        },
                    ],
                },
            },
        },
    });
    let mut body = Body::default();
    body.push_local(dummy_local(option_ty));
    let place = Place {
        local: LocalId::from_u32(0),
        projection: Box::new([PlaceElem::Downcast(VariantIdx::from_u32(1))]),
    };
    assert_eq!(m.pretty().render_place(&place, &body), "(_0 as Some)");
}

#[test]
fn render_place_deref_then_field_preserves_precedence() {
    // Specimen-26-style structure: head local is `*mut Foo`, then
    // Deref, then Field; outer parens around the deref preserve
    // deref-then-field precedence.
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mask_name = m.strings.intern("mask");
    let foo_name = m.strings.intern("Foo");
    let foo = unit_item(&mut m, ItemKind::Struct, foo_name);
    let foo_ty = m.types.intern(Ty::Struct {
        item: foo,
        args: vec![],
        layout: Layout {
            size: 4,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![FieldLayout {
                    name: mask_name,
                    ty: u32_ty,
                    offset: 0,
                }],
            },
        },
    });
    let ptr_ty = m.types.intern(Ty::Ptr(foo_ty, Mutability::Mut));
    let mut body = Body::default();
    body.push_local(dummy_local(ptr_ty));
    let place = Place {
        local: LocalId::from_u32(0),
        projection: Box::new([
            PlaceElem::Deref,
            PlaceElem::Field(FieldIdx::from_u32(0), u32_ty),
        ]),
    };
    assert_eq!(m.pretty().render_place(&place, &body), "((*_0).mask: u32)");
}

// ----- render_operand ----------------------------------------------

#[test]
fn render_operand_copy_ignores_expected_ty() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, 3, u32_ty);
    let op = Operand::Copy(Place::from(LocalId::from_u32(2)));
    assert_eq!(m.pretty().render_operand(&op, &body, u32_ty), "copy _2");
    // expected_ty arg is ignored on Copy.
    assert_eq!(
        m.pretty().render_operand(&op, &body, crate::TyId::ZERO),
        "copy _2"
    );
}

#[test]
fn render_operand_move_renders_move_prefix() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, 5, u32_ty);
    let op = Operand::Move(Place::from(LocalId::from_u32(4)));
    assert_eq!(m.pretty().render_operand(&op, &body, u32_ty), "move _4");
}

#[test]
fn render_operand_constant_threads_expected_ty_into_render_const() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let const_id = m.consts.intern(crate::Const::Uint(UintKind::U32, 42));
    let body = Body::default();
    let op = Operand::Constant(const_id);
    assert_eq!(m.pretty().render_operand(&op, &body, u32_ty), "42_u32");
}

// ----- render_rvalue -----------------------------------------------

fn body_with_u32_locals(m: &mut RegseqModel, count: usize) -> (Body, TyId) {
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, count, u32_ty);
    (body, u32_ty)
}

#[test]
fn render_rvalue_use_is_transparent() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let (body, u32_ty) = body_with_u32_locals(&mut m, 3);
    let rv = Rvalue::Use(Operand::Copy(Place::from(LocalId::from_u32(2))));
    assert_eq!(m.pretty().render_rvalue(&rv, &body, u32_ty), "copy _2");
}

#[test]
fn render_rvalue_repeat_renders_brackets_with_count() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let array_ty = m.types.intern(Ty::Array(u8_ty, 16));
    let mut body = Body::default();
    push_locals(&mut body, 5, u8_ty);
    let rv = Rvalue::Repeat(Operand::Copy(Place::from(LocalId::from_u32(3))), 16);
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, array_ty),
        "[copy _3; 16]"
    );
}

#[test]
fn render_rvalue_ref_shared_renders_amp() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let (body, u32_ty) = body_with_u32_locals(&mut m, 3);
    let ref_ty = m.types.intern(Ty::Ref(u32_ty, Provenance::SharedBorrow));
    let rv = Rvalue::Ref(Provenance::SharedBorrow, Place::from(LocalId::from_u32(2)));
    assert_eq!(m.pretty().render_rvalue(&rv, &body, ref_ty), "&_2");
}

#[test]
fn render_rvalue_ref_unique_renders_amp_mut() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let (body, u32_ty) = body_with_u32_locals(&mut m, 3);
    let ref_ty = m.types.intern(Ty::Ref(u32_ty, Provenance::UniqueBorrow));
    let rv = Rvalue::Ref(Provenance::UniqueBorrow, Place::from(LocalId::from_u32(2)));
    assert_eq!(m.pretty().render_rvalue(&rv, &body, ref_ty), "&mut _2");
}

#[test]
fn render_rvalue_rawptr_const_renders_amp_raw_const() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let (body, u32_ty) = body_with_u32_locals(&mut m, 6);
    let ptr_ty = m.types.intern(Ty::Ptr(u32_ty, Mutability::Not));
    let rv = Rvalue::RawPtr(Mutability::Not, Place::from(LocalId::from_u32(5)));
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, ptr_ty),
        "&raw const _5"
    );
}

#[test]
fn render_rvalue_rawptr_mut_renders_amp_raw_mut() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let (body, u32_ty) = body_with_u32_locals(&mut m, 6);
    let ptr_ty = m.types.intern(Ty::Ptr(u32_ty, Mutability::Mut));
    let rv = Rvalue::RawPtr(Mutability::Mut, Place::from(LocalId::from_u32(5)));
    assert_eq!(m.pretty().render_rvalue(&rv, &body, ptr_ty), "&raw mut _5");
}

#[test]
fn render_rvalue_cast_unambiguous_renders_bare() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let i64_ty = m.types.intern(Ty::Int(IntKind::I64));
    let mut body = Body::default();
    body.push_local(dummy_local(i32_ty));
    body.push_local(dummy_local(i32_ty));
    body.push_local(dummy_local(i32_ty));
    body.push_local(dummy_local(i32_ty));
    let rv = Rvalue::Cast(
        CastKind::IntToInt,
        Operand::Copy(Place::from(LocalId::from_u32(3))),
        i64_ty,
    );
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, i64_ty),
        "copy _3 as i64"
    );
}

#[test]
fn render_rvalue_cast_ambiguous_pointer_coercion_renders_annotation() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let sig = m.sigs.intern(crate::FnSig {
        inputs: vec![i32_ty],
        output: u32_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let fn_ptr_ty = m.types.intern(Ty::FnPtr(sig));
    let mut body = Body::default();
    body.push_local(dummy_local(i32_ty));
    body.push_local(dummy_local(i32_ty));
    body.push_local(dummy_local(i32_ty));
    body.push_local(dummy_local(fn_ptr_ty));
    let rv = Rvalue::Cast(
        CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer),
        Operand::Move(Place::from(LocalId::from_u32(3))),
        fn_ptr_ty,
    );
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, fn_ptr_ty),
        "move _3 as fn(i32) -> u32 (PointerCoercion(ReifyFnPointer))"
    );
}

#[test]
fn render_rvalue_cast_subtype_renders_annotation() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, 4, u32_ty);
    let rv = Rvalue::Cast(
        CastKind::Subtype,
        Operand::Copy(Place::from(LocalId::from_u32(3))),
        u32_ty,
    );
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, u32_ty),
        "copy _3 as u32 (Subtype)"
    );
}

#[test]
fn render_rvalue_binary_op_renders_infix_glyph() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let (body, u32_ty) = body_with_u32_locals(&mut m, 6);
    let rv = Rvalue::BinaryOp(
        BinOp::Add,
        Operand::Copy(Place::from(LocalId::from_u32(3))),
        Operand::Copy(Place::from(LocalId::from_u32(4))),
    );
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, u32_ty),
        "copy _3 + copy _4"
    );
}

#[test]
fn render_rvalue_unary_op_neg_renders_prefix() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let mut body = Body::default();
    push_locals(&mut body, 5, i32_ty);
    let rv = Rvalue::UnaryOp(UnOp::Neg, Operand::Copy(Place::from(LocalId::from_u32(3))));
    assert_eq!(m.pretty().render_rvalue(&rv, &body, i32_ty), "-copy _3");
}

#[test]
fn render_rvalue_discriminant_renders_call_form() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, 6, u32_ty);
    let rv = Rvalue::Discriminant(Place::from(LocalId::from_u32(5)));
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, u32_ty),
        "discriminant(_5)"
    );
}

#[test]
fn render_rvalue_aggregate_array_renders_with_elem_expected_ty() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let array_ty = m.types.intern(Ty::Array(u8_ty, 3));
    let mut body = Body::default();
    push_locals(&mut body, 4, u8_ty);
    let rv = Rvalue::Aggregate(
        AggregateKind::Array(u8_ty),
        Box::new([
            Operand::Copy(Place::from(LocalId::from_u32(1))),
            Operand::Copy(Place::from(LocalId::from_u32(2))),
            Operand::Copy(Place::from(LocalId::from_u32(3))),
        ]),
    );
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, array_ty),
        "[copy _1, copy _2, copy _3]"
    );
}

#[test]
fn render_rvalue_aggregate_tuple_arity_zero_renders_unit() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let tuple_ty = unit_ty(&mut m);
    let body = Body::default();
    let rv = Rvalue::Aggregate(AggregateKind::Tuple(tuple_ty), Box::new([]));
    assert_eq!(m.pretty().render_rvalue(&rv, &body, tuple_ty), "()");
}

#[test]
fn render_rvalue_aggregate_tuple_arity_one_renders_comma() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let tuple_ty = m.types.intern(Ty::Tuple {
        fields: vec![i32_ty],
        layout: crate::UNIT_LAYOUT,
    });
    let mut body = Body::default();
    push_locals(&mut body, 4, i32_ty);
    let rv = Rvalue::Aggregate(
        AggregateKind::Tuple(tuple_ty),
        Box::new([Operand::Copy(Place::from(LocalId::from_u32(3)))]),
    );
    assert_eq!(m.pretty().render_rvalue(&rv, &body, tuple_ty), "(copy _3,)");
}

#[test]
fn render_rvalue_aggregate_tuple_arity_two_renders_pair() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let bool_ty = m.types.intern(Ty::Bool);
    let tuple_ty = m.types.intern(Ty::Tuple {
        fields: vec![i32_ty, bool_ty],
        layout: crate::UNIT_LAYOUT,
    });
    let mut body = Body::default();
    body.push_local(dummy_local(i32_ty));
    body.push_local(dummy_local(bool_ty));
    let rv = Rvalue::Aggregate(
        AggregateKind::Tuple(tuple_ty),
        Box::new([
            Operand::Copy(Place::from(LocalId::from_u32(0))),
            Operand::Copy(Place::from(LocalId::from_u32(1))),
        ]),
    );
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, tuple_ty),
        "(copy _0, copy _1)"
    );
}

fn make_named_struct(m: &mut RegseqModel, rendered: &str, fields: Vec<(&str, TyId)>) -> TyId {
    let rendered_id = m.strings.intern(rendered);
    let item = unit_item(m, ItemKind::Struct, rendered_id);
    let field_layouts: Vec<FieldLayout> = fields
        .into_iter()
        .map(|(name, ty)| FieldLayout {
            name: m.strings.intern(name),
            ty,
            offset: 0,
        })
        .collect();
    m.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout: Layout {
            size: 0,
            align: 1,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: field_layouts,
            },
        },
    })
}

#[test]
fn render_rvalue_aggregate_struct_named_fields_renders_braces() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let struct_ty = make_named_struct(&mut m, "geom::Point", vec![("x", i32_ty), ("y", i32_ty)]);
    let mut body = Body::default();
    body.push_local(dummy_local(i32_ty));
    body.push_local(dummy_local(i32_ty));
    let rv = Rvalue::Aggregate(
        AggregateKind::Struct(struct_ty),
        Box::new([
            Operand::Copy(Place::from(LocalId::from_u32(0))),
            Operand::Copy(Place::from(LocalId::from_u32(1))),
        ]),
    );
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, struct_ty),
        "geom::Point { x: copy _0, y: copy _1 }"
    );
}

#[test]
fn render_rvalue_aggregate_struct_positional_fields_renders_parens() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let struct_ty = make_named_struct(&mut m, "Pair", vec![("0", u32_ty), ("1", u32_ty)]);
    let mut body = Body::default();
    body.push_local(dummy_local(u32_ty));
    body.push_local(dummy_local(u32_ty));
    let rv = Rvalue::Aggregate(
        AggregateKind::Struct(struct_ty),
        Box::new([
            Operand::Copy(Place::from(LocalId::from_u32(0))),
            Operand::Copy(Place::from(LocalId::from_u32(1))),
        ]),
    );
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, struct_ty),
        "Pair(copy _0, copy _1)"
    );
}

fn make_enum_with_two_variants(
    m: &mut RegseqModel,
    rendered: &str,
    args: GenericArgs,
    variants: Vec<(&str, Vec<(&str, TyId)>)>,
) -> TyId {
    let rendered_id = m.strings.intern(rendered);
    let item = unit_item(m, ItemKind::Enum, rendered_id);
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let layouts: Vec<VariantLayout> = variants
        .into_iter()
        .enumerate()
        .map(|(i, (name, fields))| {
            let field_layouts: Vec<FieldLayout> = fields
                .into_iter()
                .map(|(field_name, ty)| FieldLayout {
                    name: m.strings.intern(field_name),
                    ty,
                    offset: 0,
                })
                .collect();
            VariantLayout {
                name: m.strings.intern(name),
                fields: field_layouts,
                uninhabited: false,
                discriminant: i as u128,
                tag_value: Some(i as u128),
            }
        })
        .collect();
    m.types.intern(Ty::Enum {
        item,
        args,
        layout: Layout {
            size: 8,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Enum {
                shape: EnumShape::Multiple {
                    tag: crate::layout::TagScalar {
                        offset: 0,
                        scalar_ty: u32_ty,
                        valid_range: crate::layout::WrappingRange {
                            start: 0,
                            end: layouts.len() as u128,
                        },
                    },
                    encoding: crate::layout::TagEncoding::Direct,
                    variants: layouts,
                },
            },
        },
    })
}

#[test]
fn render_rvalue_aggregate_enum_c_style_renders_path_double_colon_variant() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let enum_ty = make_enum_with_two_variants(
        &mut m,
        "pl011::Color",
        vec![],
        vec![("Red", vec![]), ("Green", vec![])],
    );
    let body = Body::default();
    let rv = Rvalue::Aggregate(
        AggregateKind::Enum(enum_ty, VariantIdx::from_u32(0)),
        Box::new([]),
    );
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, enum_ty),
        "pl011::Color::Red"
    );
}

#[test]
fn render_rvalue_aggregate_enum_tuple_variant_renders_positional() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let enum_ty = make_enum_with_two_variants(
        &mut m,
        "core::option::Option",
        vec![GenericArg::Type(u32_ty)],
        vec![("None", vec![]), ("Some", vec![("0", u32_ty)])],
    );
    let mut body = Body::default();
    body.push_local(dummy_local(u32_ty));
    let rv = Rvalue::Aggregate(
        AggregateKind::Enum(enum_ty, VariantIdx::from_u32(1)),
        Box::new([Operand::Copy(Place::from(LocalId::from_u32(0)))]),
    );
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, enum_ty),
        "core::option::Option<u32>::Some(copy _0)"
    );
}

#[test]
fn render_rvalue_aggregate_union_renders_active_field() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let union_name = m.strings.intern("pl011::regs::MyUnion");
    let union_item = unit_item(&mut m, ItemKind::Union, union_name);
    let bits_name = m.strings.intern("bits");
    let byte_name = m.strings.intern("byte");
    let union_ty = m.types.intern(Ty::Union {
        item: union_item,
        args: vec![],
        layout: Layout {
            size: 1,
            align: 1,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Union {
                fields: vec![
                    FieldLayout {
                        name: bits_name,
                        ty: u8_ty,
                        offset: 0,
                    },
                    FieldLayout {
                        name: byte_name,
                        ty: u8_ty,
                        offset: 0,
                    },
                ],
            },
        },
    });
    let mut body = Body::default();
    body.push_local(dummy_local(u8_ty));
    let rv = Rvalue::Aggregate(
        AggregateKind::Union(union_ty, FieldIdx::from_u32(1)),
        Box::new([Operand::Copy(Place::from(LocalId::from_u32(0)))]),
    );
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, union_ty),
        "pl011::regs::MyUnion { byte: copy _0 }"
    );
}

#[test]
fn render_rvalue_aggregate_closure_renders_captures_struct_names() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    // Closure item + parallel captures struct sharing item+args identity
    // per spec.md §13.1.
    let closure_name = m.strings.intern("pl011::init::{closure#0}");
    let closure_item = unit_item(&mut m, ItemKind::Function, closure_name);
    // Closure-as-struct: a Ty::Struct with the same (item, args) as
    // the Ty::Closure carries the captures layout.
    let count_name = m.strings.intern("count");
    let _struct_ty = m.types.intern(Ty::Struct {
        item: closure_item,
        args: vec![],
        layout: Layout {
            size: 4,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![FieldLayout {
                    name: count_name,
                    ty: u32_ty,
                    offset: 0,
                }],
            },
        },
    });
    let closure_ty = m.types.intern(Ty::Closure(closure_item, vec![]));
    let mut body = Body::default();
    body.push_local(dummy_local(u32_ty));
    body.push_local(dummy_local(u32_ty));
    body.push_local(dummy_local(u32_ty));
    body.push_local(dummy_local(u32_ty));
    let rv = Rvalue::Aggregate(
        AggregateKind::Closure(closure_ty),
        Box::new([Operand::Move(Place::from(LocalId::from_u32(3)))]),
    );
    assert_eq!(
        m.pretty().render_rvalue(&rv, &body, closure_ty),
        "pl011::init::{closure#0} { count: move _3 }"
    );
}

#[test]
fn render_rvalue_aggregate_rawptr_thin_renders_unit_meta() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let unit_t = unit_ty(&mut m);
    let ptr_u8_mut = m.types.intern(Ty::Ptr(u8_ty, Mutability::Mut));
    let mut body = Body::default();
    body.push_local(dummy_local(unit_t));
    body.push_local(dummy_local(unit_t));
    body.push_local(dummy_local(unit_t));
    body.push_local(dummy_local(ptr_u8_mut));
    body.push_local(dummy_local(unit_t));
    let rv = Rvalue::Aggregate(
        AggregateKind::RawPtr(u8_ty, Mutability::Mut),
        Box::new([
            Operand::Copy(Place::from(LocalId::from_u32(3))),
            Operand::Copy(Place::from(LocalId::from_u32(4))),
        ]),
    );
    let res = m.pretty().render_rvalue(&rv, &body, ptr_u8_mut);
    assert_eq!(res, "(copy _3, copy _4) as *mut u8");
}

#[test]
fn render_rvalue_aggregate_rawptr_fat_to_slice_uses_usize_meta() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let usize_ty = m.types.intern(Ty::Uint(UintKind::Usize));
    let slice_ty = m.types.intern(Ty::Slice(u8_ty));
    let ptr_slice_const = m.types.intern(Ty::Ptr(slice_ty, Mutability::Not));
    let mut body = Body::default();
    body.push_local(dummy_local(usize_ty));
    body.push_local(dummy_local(usize_ty));
    body.push_local(dummy_local(usize_ty));
    body.push_local(dummy_local(ptr_slice_const));
    body.push_local(dummy_local(usize_ty));
    let rv = Rvalue::Aggregate(
        AggregateKind::RawPtr(slice_ty, Mutability::Not),
        Box::new([
            Operand::Copy(Place::from(LocalId::from_u32(3))),
            Operand::Copy(Place::from(LocalId::from_u32(4))),
        ]),
    );
    let res = m.pretty().render_rvalue(&rv, &body, ptr_slice_const);
    assert_eq!(res, "(copy _3, copy _4) as *const [u8]");
}

// ----- render_statement --------------------------------------------

#[test]
fn render_statement_assign_threads_place_ty_into_rvalue() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let mut body = Body::default();
    push_locals(&mut body, 6, i32_ty);
    let stmt = Statement {
        kind: StatementKind::Assign(
            Place::from(LocalId::from_u32(4)),
            Rvalue::BinaryOp(
                BinOp::Add,
                Operand::Copy(Place::from(LocalId::from_u32(3))),
                Operand::Copy(Place::from(LocalId::from_u32(5))),
            ),
        ),
        span: Span::dummy(),
    };
    assert_eq!(
        m.pretty().render_statement(&stmt, &body),
        "_4 = copy _3 + copy _5;"
    );
}

#[test]
fn render_statement_set_discriminant_renders_variant_name() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let enum_ty = make_enum_with_two_variants(
        &mut m,
        "core::option::Option",
        vec![GenericArg::Type(u32_ty)],
        vec![("None", vec![]), ("Some", vec![("0", u32_ty)])],
    );
    let mut body = Body::default();
    body.push_local(dummy_local(enum_ty));
    body.push_local(dummy_local(enum_ty));
    body.push_local(dummy_local(enum_ty));
    body.push_local(dummy_local(enum_ty));
    let stmt = Statement {
        kind: StatementKind::SetDiscriminant(
            Place::from(LocalId::from_u32(3)),
            VariantIdx::from_u32(1),
        ),
        span: Span::dummy(),
    };
    assert_eq!(
        m.pretty().render_statement(&stmt, &body),
        "discriminant(_3) = Some;"
    );
}

#[test]
fn render_statement_intrinsic_assume_renders_with_bool_expected_ty() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let bool_ty = m.types.intern(Ty::Bool);
    let mut body = Body::default();
    push_locals(&mut body, 6, bool_ty);
    let stmt = Statement {
        kind: StatementKind::Intrinsic(NonDivergingIntrinsic::Assume(Operand::Copy(Place::from(
            LocalId::from_u32(5),
        )))),
        span: Span::dummy(),
    };
    assert_eq!(
        m.pretty().render_statement(&stmt, &body),
        "assume(copy _5);"
    );
}

#[test]
fn render_statement_intrinsic_copy_non_overlapping_renders_three_args() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let usize_ty = m.types.intern(Ty::Uint(UintKind::Usize));
    let ptr_const = m.types.intern(Ty::Ptr(u8_ty, Mutability::Not));
    let ptr_mut = m.types.intern(Ty::Ptr(u8_ty, Mutability::Mut));
    let mut body = Body::default();
    body.push_local(dummy_local(usize_ty));
    body.push_local(dummy_local(usize_ty));
    body.push_local(dummy_local(usize_ty));
    body.push_local(dummy_local(ptr_const));
    body.push_local(dummy_local(ptr_mut));
    body.push_local(dummy_local(usize_ty));
    let stmt = Statement {
        kind: StatementKind::Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(
            CopyNonOverlapping {
                src: Operand::Move(Place::from(LocalId::from_u32(3))),
                dst: Operand::Move(Place::from(LocalId::from_u32(4))),
                count: Operand::Copy(Place::from(LocalId::from_u32(5))),
            },
        )),
        span: Span::dummy(),
    };
    assert_eq!(
        m.pretty().render_statement(&stmt, &body),
        "copy_nonoverlapping(move _3, move _4, copy _5);"
    );
}

// ----- render_terminator -------------------------------------------

#[test]
fn render_terminator_goto_renders_single_successor() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let body = Body::default();
    let term = Terminator {
        kind: TerminatorKind::Goto(BlockId::from_u32(3)),
        span: Span::dummy(),
    };
    assert_eq!(m.pretty().render_terminator(&term, &body), "goto -> [bb3]");
}

#[test]
fn render_terminator_return_renders_empty_successor_list() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let body = Body::default();
    let term = Terminator {
        kind: TerminatorKind::Return,
        span: Span::dummy(),
    };
    assert_eq!(m.pretty().render_terminator(&term, &body), "return -> []");
}

#[test]
fn render_terminator_unreachable_renders_empty_successor_list() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let body = Body::default();
    let term = Terminator {
        kind: TerminatorKind::Unreachable,
        span: Span::dummy(),
    };
    assert_eq!(
        m.pretty().render_terminator(&term, &body),
        "unreachable -> []"
    );
}

#[test]
fn render_terminator_switch_int_renders_targets() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, 4, u32_ty);
    let term = Terminator {
        kind: TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::from(LocalId::from_u32(3))),
            targets: SwitchTargets {
                cases: Box::new([(0, BlockId::from_u32(1)), (1, BlockId::from_u32(2))]),
                otherwise: BlockId::from_u32(3),
            },
        },
        span: Span::dummy(),
    };
    assert_eq!(
        m.pretty().render_terminator(&term, &body),
        "switchInt(copy _3) -> [0: bb1, 1: bb2, otherwise: bb3]"
    );
}

#[test]
fn render_terminator_call_direct_renders_return_target() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let bool_ty = m.types.intern(Ty::Bool);
    let write_byte_name = m.strings.intern("pl011::write_byte");
    let item = unit_item(&mut m, ItemKind::Function, write_byte_name);
    let sig = m.sigs.intern(crate::FnSig {
        inputs: vec![u8_ty, u8_ty],
        output: bool_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let emission = m.strings.intern("pl011_write_byte");
    let instance_id = m.instances.push(Instance {
        item,
        generic_args: vec![GenericArg::Type(u8_ty)],
        emission_name: emission,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig,
            body: Some(Body::default()),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    let mut body = Body::default();
    body.push_local(dummy_local(bool_ty));
    body.push_local(dummy_local(u8_ty));
    body.push_local(dummy_local(u8_ty));
    body.push_local(dummy_local(u8_ty));
    body.push_local(dummy_local(u8_ty));
    let term = Terminator {
        kind: TerminatorKind::Call {
            callee: CallCallee::Direct(instance_id),
            args: Box::new([
                Operand::Move(Place::from(LocalId::from_u32(3))),
                Operand::Copy(Place::from(LocalId::from_u32(4))),
            ]),
            destination: Place::from(LocalId::from_u32(0)),
            target: Some(BlockId::from_u32(2)),
        },
        span: Span::dummy(),
    };
    assert_eq!(
        m.pretty().render_terminator(&term, &body),
        "_0 = pl011::write_byte<u8>(move _3, copy _4) -> [return: bb2]"
    );
}

#[test]
fn render_terminator_call_indirect_renders_operand_callee() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let bool_ty = m.types.intern(Ty::Bool);
    let sig = m.sigs.intern(crate::FnSig {
        inputs: vec![u8_ty],
        output: bool_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let fn_ptr_ty = m.types.intern(Ty::FnPtr(sig));
    let mut body = Body::default();
    body.push_local(dummy_local(bool_ty));
    body.push_local(dummy_local(u8_ty));
    body.push_local(dummy_local(u8_ty));
    body.push_local(dummy_local(u8_ty));
    body.push_local(dummy_local(u8_ty));
    body.push_local(dummy_local(fn_ptr_ty));
    let term = Terminator {
        kind: TerminatorKind::Call {
            callee: CallCallee::Indirect(Operand::Copy(Place::from(LocalId::from_u32(5)))),
            args: Box::new([Operand::Move(Place::from(LocalId::from_u32(3)))]),
            destination: Place::from(LocalId::from_u32(0)),
            target: Some(BlockId::from_u32(2)),
        },
        span: Span::dummy(),
    };
    assert_eq!(
        m.pretty().render_terminator(&term, &body),
        "_0 = copy _5(move _3) -> [return: bb2]"
    );
}

#[test]
fn render_terminator_call_diverging_has_empty_successor_list() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let bool_ty = m.types.intern(Ty::Bool);
    let sig = m.sigs.intern(crate::FnSig {
        inputs: vec![],
        output: bool_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let abort_name = m.strings.intern("abort");
    let item = unit_item(&mut m, ItemKind::Function, abort_name);
    let emission = m.strings.intern("abort");
    let instance_id = m.instances.push(Instance {
        item,
        generic_args: vec![],
        emission_name: emission,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig,
            body: Some(Body::default()),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    let mut body = Body::default();
    body.push_local(dummy_local(u8_ty));
    let term = Terminator {
        kind: TerminatorKind::Call {
            callee: CallCallee::Direct(instance_id),
            args: Box::new([]),
            destination: Place::from(LocalId::from_u32(0)),
            target: None,
        },
        span: Span::dummy(),
    };
    assert_eq!(
        m.pretty().render_terminator(&term, &body),
        "_0 = abort() -> []"
    );
}

#[test]
fn render_terminator_assert_renders_message_and_success_target() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let bool_ty = m.types.intern(Ty::Bool);
    let usize_ty = m.types.intern(Ty::Uint(UintKind::Usize));
    let mut body = Body::default();
    body.push_local(dummy_local(bool_ty));
    body.push_local(dummy_local(bool_ty));
    body.push_local(dummy_local(bool_ty));
    body.push_local(dummy_local(bool_ty));
    body.push_local(dummy_local(usize_ty));
    body.push_local(dummy_local(usize_ty));
    let term = Terminator {
        kind: TerminatorKind::Assert {
            cond: Operand::Copy(Place::from(LocalId::from_u32(3))),
            expected: true,
            msg: AssertKind::BoundsCheck {
                len: Operand::Copy(Place::from(LocalId::from_u32(4))),
                index: Operand::Copy(Place::from(LocalId::from_u32(5))),
            },
            target: BlockId::from_u32(2),
        },
        span: Span::dummy(),
    };
    assert_eq!(
        m.pretty().render_terminator(&term, &body),
        "assert(true, copy _3, index out of bounds: the length is copy _4 but the index is copy _5) -> [success: bb2]"
    );
}

// ----- render_call_callee ------------------------------------------

#[test]
fn render_call_callee_direct_renders_path_and_generic_args() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let bool_ty = m.types.intern(Ty::Bool);
    let write_byte_name = m.strings.intern("pl011::write_byte");
    let item = unit_item(&mut m, ItemKind::Function, write_byte_name);
    let sig = m.sigs.intern(crate::FnSig {
        inputs: vec![u8_ty],
        output: bool_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let emission = m.strings.intern("pl011_write_byte");
    let instance_id = m.instances.push(Instance {
        item,
        generic_args: vec![GenericArg::Type(u8_ty)],
        emission_name: emission,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig,
            body: Some(Body::default()),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    let body = Body::default();
    assert_eq!(
        m.pretty()
            .render_call_callee(&CallCallee::Direct(instance_id), &body),
        "pl011::write_byte<u8>"
    );
}

#[test]
fn render_call_callee_indirect_renders_operand() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let bool_ty = m.types.intern(Ty::Bool);
    let sig = m.sigs.intern(crate::FnSig {
        inputs: vec![u8_ty],
        output: bool_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let fn_ptr_ty = m.types.intern(Ty::FnPtr(sig));
    let mut body = Body::default();
    push_locals(&mut body, 5, u8_ty);
    body.push_local(dummy_local(fn_ptr_ty));
    let callee = CallCallee::Indirect(Operand::Copy(Place::from(LocalId::from_u32(5))));
    assert_eq!(m.pretty().render_call_callee(&callee, &body), "copy _5");
}

// ----- render_switch_targets ---------------------------------------

#[test]
fn render_switch_targets_zero_cases_renders_otherwise_only() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let body = Body::default();
    let targets = SwitchTargets {
        cases: Box::new([]),
        otherwise: BlockId::from_u32(7),
    };
    assert_eq!(
        m.pretty().render_switch_targets(&targets, &body),
        "otherwise: bb7"
    );
}

#[test]
fn render_switch_targets_multi_cases_render_commas() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let body = Body::default();
    let targets = SwitchTargets {
        cases: Box::new([
            (0u128, BlockId::from_u32(1)),
            (1u128, BlockId::from_u32(2)),
            (5u128, BlockId::from_u32(7)),
        ]),
        otherwise: BlockId::from_u32(3),
    };
    assert_eq!(
        m.pretty().render_switch_targets(&targets, &body),
        "0: bb1, 1: bb2, 5: bb7, otherwise: bb3"
    );
}

// ----- render_assert_kind ------------------------------------------

#[test]
fn render_assert_kind_bounds_check_template() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let usize_ty = m.types.intern(Ty::Uint(UintKind::Usize));
    let mut body = Body::default();
    push_locals(&mut body, 6, usize_ty);
    let kind = AssertKind::BoundsCheck {
        len: Operand::Copy(Place::from(LocalId::from_u32(4))),
        index: Operand::Copy(Place::from(LocalId::from_u32(5))),
    };
    assert_eq!(
        m.pretty().render_assert_kind(&kind, &body),
        "index out of bounds: the length is copy _4 but the index is copy _5"
    );
}

#[test]
fn render_assert_kind_overflow_add_template() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let mut body = Body::default();
    push_locals(&mut body, 6, i32_ty);
    let kind = AssertKind::Overflow(
        BinOp::Add,
        Operand::Copy(Place::from(LocalId::from_u32(4))),
        Operand::Copy(Place::from(LocalId::from_u32(5))),
    );
    assert_eq!(
        m.pretty().render_assert_kind(&kind, &body),
        "attempt to compute `copy _4 + copy _5`, which would overflow"
    );
}

#[test]
fn render_assert_kind_overflow_shl_template() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    body.push_local(dummy_local(i32_ty));
    body.push_local(dummy_local(i32_ty));
    body.push_local(dummy_local(i32_ty));
    body.push_local(dummy_local(i32_ty));
    body.push_local(dummy_local(i32_ty));
    body.push_local(dummy_local(u32_ty));
    let kind = AssertKind::Overflow(
        BinOp::Shl,
        Operand::Copy(Place::from(LocalId::from_u32(4))),
        Operand::Copy(Place::from(LocalId::from_u32(5))),
    );
    assert_eq!(
        m.pretty().render_assert_kind(&kind, &body),
        "attempt to shift left by `copy _5`, which would overflow"
    );
}

#[test]
fn render_assert_kind_overflow_neg_template() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let mut body = Body::default();
    push_locals(&mut body, 5, i32_ty);
    let kind = AssertKind::OverflowNeg(Operand::Copy(Place::from(LocalId::from_u32(4))));
    assert_eq!(
        m.pretty().render_assert_kind(&kind, &body),
        "attempt to negate `copy _4`, which would overflow"
    );
}

#[test]
fn render_assert_kind_division_by_zero_template() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let mut body = Body::default();
    push_locals(&mut body, 5, i32_ty);
    let kind = AssertKind::DivisionByZero(Operand::Copy(Place::from(LocalId::from_u32(4))));
    assert_eq!(
        m.pretty().render_assert_kind(&kind, &body),
        "attempt to divide `copy _4` by zero"
    );
}

#[test]
fn render_assert_kind_remainder_by_zero_template() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let mut body = Body::default();
    push_locals(&mut body, 5, i32_ty);
    let kind = AssertKind::RemainderByZero(Operand::Copy(Place::from(LocalId::from_u32(4))));
    assert_eq!(
        m.pretty().render_assert_kind(&kind, &body),
        "attempt to calculate the remainder of `copy _4` with a divisor of zero"
    );
}

#[test]
fn render_assert_kind_misaligned_pointer_dereference_template() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let usize_ty = m.types.intern(Ty::Uint(UintKind::Usize));
    let mut body = Body::default();
    push_locals(&mut body, 6, usize_ty);
    let kind = AssertKind::MisalignedPointerDereference {
        required: Operand::Copy(Place::from(LocalId::from_u32(4))),
        found: Operand::Copy(Place::from(LocalId::from_u32(5))),
    };
    assert_eq!(
        m.pretty().render_assert_kind(&kind, &body),
        "misaligned pointer dereference: address must be a multiple of copy _4 but is copy _5"
    );
}

#[test]
fn render_assert_kind_null_pointer_dereference_is_constant_message() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let body = Body::default();
    assert_eq!(
        m.pretty()
            .render_assert_kind(&AssertKind::NullPointerDereference, &body),
        "null pointer dereference occurred"
    );
}

#[test]
fn render_assert_kind_invalid_enum_construction_template() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, 5, u32_ty);
    let kind =
        AssertKind::InvalidEnumConstruction(Operand::Copy(Place::from(LocalId::from_u32(4))));
    assert_eq!(
        m.pretty().render_assert_kind(&kind, &body),
        "invalid enum discriminant value: copy _4"
    );
}

// ----- render_basic_block ------------------------------------------

#[test]
fn render_basic_block_empty_statements_emits_terminator_only() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let mut body = Body::default();
    // Push a filler block at index 0 so the rendered block's
    // positional BlockId is 1 (matches the previous specimen).
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind: TerminatorKind::Return,
            span: Span::dummy(),
        },
    });
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind: TerminatorKind::Goto(BlockId::from_u32(2)),
            span: Span::dummy(),
        },
    });
    let block = &body.basic_blocks[BlockId::from_u32(1)];
    let out = m.pretty().render_basic_block(block, &body);
    assert_eq!(out, "    bb1: {\n        goto -> [bb2];\n    }");
}

#[test]
fn render_basic_block_multi_statement_uses_4_and_8_space_indent() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let mut body = Body::default();
    push_locals(&mut body, 6, i32_ty);
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([
            Statement {
                kind: StatementKind::Assign(
                    Place::from(LocalId::from_u32(3)),
                    Rvalue::Use(Operand::Copy(Place::from(LocalId::from_u32(4)))),
                ),
                span: Span::dummy(),
            },
            Statement {
                kind: StatementKind::Assign(
                    Place::from(LocalId::from_u32(4)),
                    Rvalue::BinaryOp(
                        BinOp::Add,
                        Operand::Copy(Place::from(LocalId::from_u32(3))),
                        Operand::Copy(Place::from(LocalId::from_u32(5))),
                    ),
                ),
                span: Span::dummy(),
            },
        ]),
        terminator: Terminator {
            kind: TerminatorKind::Return,
            span: Span::dummy(),
        },
    });
    let block = &body.basic_blocks[BlockId::from_u32(0)];
    let out = m.pretty().render_basic_block(block, &body);
    assert_eq!(
        out,
        "    bb0: {\n        _3 = copy _4;\n        _4 = copy _3 + copy _5;\n        return -> [];\n    }"
    );
}

// ----- §4 body-level span gating ----------------------------------

#[test]
fn render_statement_default_options_emits_no_span_comment() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let mut body = Body::default();
    push_locals(&mut body, 4, i32_ty);
    let path = m.strings.intern("pl011.rs");
    let file_id = m.source_files.push(SourceFile {
        path,
        content: Some("let x = 1;\n".into()),
        line_starts: vec![0],
    });
    let stmt = Statement {
        kind: StatementKind::Assign(
            Place::from(LocalId::from_u32(3)),
            Rvalue::Use(Operand::Copy(Place::from(LocalId::from_u32(2)))),
        ),
        span: Span {
            file: file_id,
            lo: 0,
            hi: 5,
        },
    };
    assert_eq!(m.pretty().render_statement(&stmt, &body), "_3 = copy _2;");
}

#[test]
fn render_statement_include_spans_emits_aligned_trailing_comment() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let mut body = Body::default();
    push_locals(&mut body, 4, i32_ty);
    let path = m.strings.intern("pl011.rs");
    let file_id = m.source_files.push(SourceFile {
        path,
        content: Some("let x = 1;\n".into()),
        line_starts: vec![0],
    });
    let stmt = Statement {
        kind: StatementKind::Assign(
            Place::from(LocalId::from_u32(3)),
            Rvalue::Use(Operand::Copy(Place::from(LocalId::from_u32(2)))),
        ),
        span: Span {
            file: file_id,
            lo: 0,
            hi: 5,
        },
    };
    let opts = PrettyPrintOptions {
        include_spans: true,
        include_debug_names: false,
    };
    let rendered = PrettyPrinter::with_options(&m, opts).render_statement(&stmt, &body);
    let head = "_3 = copy _2;";
    let expected = format!("{head:<40} // pl011.rs:1:1: 1:6");
    assert_eq!(rendered, expected);
}

#[test]
fn render_statement_include_spans_suppresses_synthesized_span() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let mut body = Body::default();
    push_locals(&mut body, 4, i32_ty);
    let stmt = Statement {
        kind: StatementKind::Assign(
            Place::from(LocalId::from_u32(3)),
            Rvalue::Use(Operand::Copy(Place::from(LocalId::from_u32(2)))),
        ),
        span: Span::dummy(),
    };
    let opts = PrettyPrintOptions {
        include_spans: true,
        include_debug_names: false,
    };
    let rendered = PrettyPrinter::with_options(&m, opts).render_statement(&stmt, &body);
    assert_eq!(rendered, "_3 = copy _2;");
}

#[test]
fn render_terminator_include_spans_emits_aligned_trailing_comment() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let body = Body::default();
    let path = m.strings.intern("pl011.rs");
    let file_id = m.source_files.push(SourceFile {
        path,
        content: Some("return\n".into()),
        line_starts: vec![0],
    });
    let term = Terminator {
        kind: TerminatorKind::Return,
        span: Span {
            file: file_id,
            lo: 0,
            hi: 6,
        },
    };
    let opts = PrettyPrintOptions {
        include_spans: true,
        include_debug_names: false,
    };
    let rendered = PrettyPrinter::with_options(&m, opts).render_terminator(&term, &body);
    let head = "return -> []";
    let expected = format!("{head:<40} // pl011.rs:1:1: 1:7");
    assert_eq!(rendered, expected);
}

#[test]
fn render_terminator_include_spans_suppresses_synthesized_span() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let body = Body::default();
    let term = Terminator {
        kind: TerminatorKind::Return,
        span: Span::dummy(),
    };
    let opts = PrettyPrintOptions {
        include_spans: true,
        include_debug_names: false,
    };
    let rendered = PrettyPrinter::with_options(&m, opts).render_terminator(&term, &body);
    assert_eq!(rendered, "return -> []");
}

// ------------------------------------------------------------------
// render_const: P1a-PP-04a per-arm dispatch (scalar / ZST).
//
// Spec coverage:
// - §10.1.1 Bool / Char / Int / Uint / Float scalar dispatch.
// - D-14-4.10 sentinel collapsing for Int / Uint widths.
// - D-14-4.12 non-finite Float classification + single-form NaN.
// - §10.1.2 ZST per-Ty.kind() dispatch and composite recursion.
// ------------------------------------------------------------------

use crate::const_value::Const;
use crate::layout::{TagEncoding, TagScalar, WrappingRange};
use crate::target::default_scalar_layouts;
use crate::UNIT_LAYOUT;

// ----- Bool ---------------------------------------------------------

#[test]
fn render_const_bool_true_renders_without_suffix() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let bool_ty = m.types.intern(Ty::Bool);
    let id = m.consts.intern(Const::Bool(true));
    assert_eq!(m.pretty().render_const(id, bool_ty), "true");
}

#[test]
fn render_const_bool_false_renders_without_suffix() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let bool_ty = m.types.intern(Ty::Bool);
    let id = m.consts.intern(Const::Bool(false));
    assert_eq!(m.pretty().render_const(id, bool_ty), "false");
}

// ----- Char ---------------------------------------------------------

#[test]
fn render_const_char_ascii_printable_renders_via_debug() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let char_ty = m.types.intern(Ty::Char);
    let id = m.consts.intern(Const::Char(0x61));
    assert_eq!(m.pretty().render_const(id, char_ty), "'a'");
}

#[test]
fn render_const_char_escape_renders_via_debug() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let char_ty = m.types.intern(Ty::Char);
    let id = m.consts.intern(Const::Char(0x0A));
    assert_eq!(m.pretty().render_const(id, char_ty), "'\\n'");
}

#[test]
fn render_const_char_non_ascii_renders_unicode_escape() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let char_ty = m.types.intern(Ty::Char);
    // U+1F600 -- 😀 -- standard Rust char Debug emits the literal,
    // not the `\u{...}` escape, because it's printable. Pick a
    // control point that is not printable to exercise the escape
    // form. U+007F (DEL) renders as '\u{7f}' per char::Debug.
    let id = m.consts.intern(Const::Char(0x7F));
    assert_eq!(m.pretty().render_const(id, char_ty), "'\\u{7f}'");
}

// ----- Int -- decimal form -----------------------------------------

#[test]
fn render_const_int_i32_non_sentinel_renders_underscore_decimal() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Int(IntKind::I32));
    let id = m.consts.intern(Const::Int(IntKind::I32, 42));
    assert_eq!(m.pretty().render_const(id, ty), "42_i32");
}

#[test]
fn render_const_int_i8_sign_extended_negative_renders_decimal() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Int(IntKind::I8));
    // (-1_i8 as i128) as u128 -- sign-extended storage per spec §9.7.
    let id = m
        .consts
        .intern(Const::Int(IntKind::I8, (-1_i8 as i128) as u128));
    assert_eq!(m.pretty().render_const(id, ty), "-1_i8");
}

#[test]
fn render_const_int_each_kind_renders_with_kind_suffix() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i8_ty = m.types.intern(Ty::Int(IntKind::I8));
    let i16_ty = m.types.intern(Ty::Int(IntKind::I16));
    let i64_ty = m.types.intern(Ty::Int(IntKind::I64));
    let i128_ty = m.types.intern(Ty::Int(IntKind::I128));
    let isize_ty = m.types.intern(Ty::Int(IntKind::Isize));
    let i8 = m.consts.intern(Const::Int(IntKind::I8, 7));
    let i16 = m.consts.intern(Const::Int(IntKind::I16, 1000));
    let i64 = m.consts.intern(Const::Int(IntKind::I64, 999999));
    let i128 = m.consts.intern(Const::Int(IntKind::I128, 1));
    let isize = m.consts.intern(Const::Int(IntKind::Isize, 16));
    let p = m.pretty();
    assert_eq!(p.render_const(i8, i8_ty), "7_i8");
    assert_eq!(p.render_const(i16, i16_ty), "1000_i16");
    assert_eq!(p.render_const(i64, i64_ty), "999999_i64");
    assert_eq!(p.render_const(i128, i128_ty), "1_i128");
    assert_eq!(p.render_const(isize, isize_ty), "16_isize");
}

// ----- Int / Uint -- D-14-4.10 sentinel collapsing -----------------

#[test]
fn render_const_int_i32_max_collapses_to_sentinel_no_suffix() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Int(IntKind::I32));
    let id = m
        .consts
        .intern(Const::Int(IntKind::I32, i32::MAX as i128 as u128));
    assert_eq!(m.pretty().render_const(id, ty), "i32::MAX");
}

#[test]
fn render_const_int_i32_min_collapses_to_sentinel() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Int(IntKind::I32));
    let id = m
        .consts
        .intern(Const::Int(IntKind::I32, i32::MIN as i128 as u128));
    assert_eq!(m.pretty().render_const(id, ty), "i32::MIN");
}

#[test]
fn render_const_int_i8_min_max_collapse_to_sentinel() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Int(IntKind::I8));
    let min = m
        .consts
        .intern(Const::Int(IntKind::I8, i8::MIN as i128 as u128));
    let max = m
        .consts
        .intern(Const::Int(IntKind::I8, i8::MAX as i128 as u128));
    let p = m.pretty();
    assert_eq!(p.render_const(min, ty), "i8::MIN");
    assert_eq!(p.render_const(max, ty), "i8::MAX");
}

#[test]
fn render_const_int_i128_min_max_collapse_to_sentinel() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Int(IntKind::I128));
    let min = m
        .consts
        .intern(Const::Int(IntKind::I128, i128::MIN as u128));
    let max = m
        .consts
        .intern(Const::Int(IntKind::I128, i128::MAX as u128));
    let p = m.pretty();
    assert_eq!(p.render_const(min, ty), "i128::MIN");
    assert_eq!(p.render_const(max, ty), "i128::MAX");
}

#[test]
fn render_const_uint_u8_max_collapses_to_sentinel() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Uint(UintKind::U8));
    let id = m.consts.intern(Const::Uint(UintKind::U8, 255));
    assert_eq!(m.pretty().render_const(id, ty), "u8::MAX");
}

#[test]
fn render_const_uint_u128_max_collapses_to_sentinel() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Uint(UintKind::U128));
    let id = m.consts.intern(Const::Uint(UintKind::U128, u128::MAX));
    assert_eq!(m.pretty().render_const(id, ty), "u128::MAX");
}

#[test]
fn render_const_uint_zero_does_not_collapse_to_min_sentinel() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Uint(UintKind::U8));
    let id = m.consts.intern(Const::Uint(UintKind::U8, 0));
    // Per D-14-4.10: no unsigned MIN collapsing; renders as decimal.
    assert_eq!(m.pretty().render_const(id, ty), "0_u8");
}

#[test]
fn render_const_uint_non_sentinel_renders_underscore_decimal() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Uint(UintKind::U32));
    let id = m.consts.intern(Const::Uint(UintKind::U32, 0xDEAD_BEEF));
    assert_eq!(m.pretty().render_const(id, ty), "3735928559_u32");
}

// ----- Int / Uint -- Isize / Usize target-pointer-width-independent

fn model_with_pointer_width(pw: u8) -> RegseqModel {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    m.target.pointer_width = pw;
    m.target.scalar_layouts = default_scalar_layouts(pw);
    m
}

#[test]
fn render_const_usize_max_renders_symbolically_on_32_bit_target() {
    let mut m = model_with_pointer_width(32);
    let ty = m.types.intern(Ty::Uint(UintKind::Usize));
    let id = m.consts.intern(Const::Uint(UintKind::Usize, 0xFFFFFFFF));
    assert_eq!(m.pretty().render_const(id, ty), "usize::MAX");
}

#[test]
fn render_const_usize_max_renders_symbolically_on_64_bit_target() {
    let mut m = model_with_pointer_width(64);
    let ty = m.types.intern(Ty::Uint(UintKind::Usize));
    let id = m
        .consts
        .intern(Const::Uint(UintKind::Usize, 0xFFFFFFFF_FFFFFFFF));
    assert_eq!(m.pretty().render_const(id, ty), "usize::MAX");
}

#[test]
fn render_const_isize_min_max_render_symbolically_on_both_targets() {
    let mut m32 = model_with_pointer_width(32);
    let ty32 = m32.types.intern(Ty::Int(IntKind::Isize));
    let min32 = m32
        .consts
        .intern(Const::Int(IntKind::Isize, i32::MIN as i128 as u128));
    let max32 = m32
        .consts
        .intern(Const::Int(IntKind::Isize, i32::MAX as i128 as u128));
    let p = m32.pretty();
    assert_eq!(p.render_const(min32, ty32), "isize::MIN");
    assert_eq!(p.render_const(max32, ty32), "isize::MAX");

    let mut m64 = model_with_pointer_width(64);
    let ty64 = m64.types.intern(Ty::Int(IntKind::Isize));
    let min64 = m64
        .consts
        .intern(Const::Int(IntKind::Isize, i64::MIN as i128 as u128));
    let max64 = m64
        .consts
        .intern(Const::Int(IntKind::Isize, i64::MAX as i128 as u128));
    let p = m64.pretty();
    assert_eq!(p.render_const(min64, ty64), "isize::MIN");
    assert_eq!(p.render_const(max64, ty64), "isize::MAX");
}

// ----- Float -- finite forms ---------------------------------------

#[test]
fn render_const_float_f32_integer_valued_renders_without_fractional() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Float(FloatKind::F32));
    let id = m
        .consts
        .intern(Const::Float(FloatKind::F32, 1.0f32.to_bits() as u128));
    assert_eq!(m.pretty().render_const(id, ty), "1f32");
}

#[test]
fn render_const_float_f32_non_integer_renders_with_fractional() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Float(FloatKind::F32));
    let id = m
        .consts
        .intern(Const::Float(FloatKind::F32, 1.5f32.to_bits() as u128));
    assert_eq!(m.pretty().render_const(id, ty), "1.5f32");
}

#[test]
fn render_const_float_f64_negative_zero_finite_renders_as_minus_zero() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Float(FloatKind::F32));
    // -0f32 bits = 0x80000000.
    let id = m.consts.intern(Const::Float(FloatKind::F32, 0x8000_0000));
    assert_eq!(m.pretty().render_const(id, ty), "-0f32");
}

#[test]
fn render_const_float_f64_integer_renders_without_fractional() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Float(FloatKind::F64));
    let id = m
        .consts
        .intern(Const::Float(FloatKind::F64, 42.0f64.to_bits() as u128));
    assert_eq!(m.pretty().render_const(id, ty), "42f64");
}

#[test]
fn render_const_float_f16_integer_value_renders_with_f16_suffix() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Float(FloatKind::F16));
    // f16 1.0 bit pattern: sign=0, exp=15 (biased), mantissa=0
    // -> 0 011 1100 0000 0000 = 0x3C00.
    let id = m.consts.intern(Const::Float(FloatKind::F16, 0x3C00));
    assert_eq!(m.pretty().render_const(id, ty), "1f16");
}

#[test]
fn render_const_float_f128_integer_value_renders_with_f128_suffix() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Float(FloatKind::F128));
    // f128 1.0: sign=0, exp=16383 (biased), mantissa=0.
    let bits = 0x3FFFu128 << 112;
    let id = m.consts.intern(Const::Float(FloatKind::F128, bits));
    assert_eq!(m.pretty().render_const(id, ty), "1f128");
}

#[test]
fn render_const_float_f128_non_integer_value_renders_with_fractional() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Float(FloatKind::F128));
    // f128 1.5: sign=0, exp=16383 (biased), mantissa = 1 << 111 (the
    // leading mantissa bit). The apfloat-based renderer produces the
    // shortest round-trippable decimal display per pretty-printer.md
    // §10.1.1 even for widths Rust has no native type for.
    let bits = (0x3FFFu128 << 112) | (1u128 << 111);
    let id = m.consts.intern(Const::Float(FloatKind::F128, bits));
    assert_eq!(m.pretty().render_const(id, ty), "1.5f128");
}

#[test]
fn render_const_float_f16_non_integer_value_renders_with_fractional() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Float(FloatKind::F16));
    // f16 1.5 bit pattern: sign=0, exp=15 (biased), mantissa = 1<<9.
    let bits = 0x3E00u128;
    let id = m.consts.intern(Const::Float(FloatKind::F16, bits));
    assert_eq!(m.pretty().render_const(id, ty), "1.5f16");
}

// ----- Float -- D-14-4.12 non-finite ------------------------------

#[test]
fn render_const_float_f32_positive_infinity() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Float(FloatKind::F32));
    let id = m.consts.intern(Const::Float(FloatKind::F32, 0x7F80_0000));
    assert_eq!(m.pretty().render_const(id, ty), "inf_f32");
}

#[test]
fn render_const_float_f32_negative_infinity() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Float(FloatKind::F32));
    let id = m.consts.intern(Const::Float(FloatKind::F32, 0xFF80_0000));
    assert_eq!(m.pretty().render_const(id, ty), "-inf_f32");
}

#[test]
fn render_const_float_f32_all_nan_bit_patterns_collapse_to_single_form() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let ty = m.types.intern(Ty::Float(FloatKind::F32));
    let quiet = m.consts.intern(Const::Float(FloatKind::F32, 0x7FC0_0000));
    let signaling = m.consts.intern(Const::Float(FloatKind::F32, 0x7F80_0001));
    let payload = m.consts.intern(Const::Float(FloatKind::F32, 0x7FC0_1234));
    let sign_set = m.consts.intern(Const::Float(FloatKind::F32, 0xFFC0_0000));
    let p = m.pretty();
    assert_eq!(p.render_const(quiet, ty), "NaN_f32");
    assert_eq!(p.render_const(signaling, ty), "NaN_f32");
    assert_eq!(p.render_const(payload, ty), "NaN_f32");
    assert_eq!(p.render_const(sign_set, ty), "NaN_f32");
}

#[test]
fn render_const_float_each_width_non_finite_classifications() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let f16_ty = m.types.intern(Ty::Float(FloatKind::F16));
    let f64_ty = m.types.intern(Ty::Float(FloatKind::F64));
    let f128_ty = m.types.intern(Ty::Float(FloatKind::F128));

    // f16: exp 0x1F all-ones, mantissa 0 -> ±inf; mantissa nonzero -> NaN.
    let f16_inf = m.consts.intern(Const::Float(FloatKind::F16, 0x7C00));
    let f16_neg_inf = m.consts.intern(Const::Float(FloatKind::F16, 0xFC00));
    let f16_nan = m.consts.intern(Const::Float(FloatKind::F16, 0x7E00));
    // f64: 0x7FF0_0000_0000_0000 / 0xFFF0_..._0000 / NaN.
    let f64_inf = m
        .consts
        .intern(Const::Float(FloatKind::F64, 0x7FF0_0000_0000_0000));
    let f64_neg_inf = m
        .consts
        .intern(Const::Float(FloatKind::F64, 0xFFF0_0000_0000_0000));
    let f64_nan = m
        .consts
        .intern(Const::Float(FloatKind::F64, 0x7FF8_0000_0000_0000));
    // f128: 0x7FFF_<all-zero 112-bit mantissa> -> +inf, 0xFFFF_<0> -> -inf, mantissa != 0 -> NaN.
    let f128_inf = m
        .consts
        .intern(Const::Float(FloatKind::F128, 0x7FFFu128 << 112));
    let f128_neg_inf = m
        .consts
        .intern(Const::Float(FloatKind::F128, 0xFFFFu128 << 112));
    let f128_nan = m
        .consts
        .intern(Const::Float(FloatKind::F128, (0x7FFFu128 << 112) | 1));

    let p = m.pretty();
    assert_eq!(p.render_const(f16_inf, f16_ty), "inf_f16");
    assert_eq!(p.render_const(f16_neg_inf, f16_ty), "-inf_f16");
    assert_eq!(p.render_const(f16_nan, f16_ty), "NaN_f16");
    assert_eq!(p.render_const(f64_inf, f64_ty), "inf_f64");
    assert_eq!(p.render_const(f64_neg_inf, f64_ty), "-inf_f64");
    assert_eq!(p.render_const(f64_nan, f64_ty), "NaN_f64");
    assert_eq!(p.render_const(f128_inf, f128_ty), "inf_f128");
    assert_eq!(p.render_const(f128_neg_inf, f128_ty), "-inf_f128");
    assert_eq!(p.render_const(f128_nan, f128_ty), "NaN_f128");
}

// ----- ZST -- §10.1.2 dispatch ------------------------------------

#[test]
fn render_const_zst_unit_renders_as_unit_paren_pair() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    });
    let id = m.consts.intern(Const::ZST);
    assert_eq!(m.pretty().render_const(id, unit), "()");
}

#[test]
fn render_const_zst_array_zero_length_renders_empty_brackets() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let array_ty = m.types.intern(Ty::Array(i32_ty, 0));
    let id = m.consts.intern(Const::ZST);
    assert_eq!(m.pretty().render_const(id, array_ty), "[]");
}

#[test]
fn render_const_zst_array_zst_elem_repeats_count_decimal() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    });
    let array_ty = m.types.intern(Ty::Array(unit, 100));
    let id = m.consts.intern(Const::ZST);
    assert_eq!(m.pretty().render_const(id, array_ty), "[(); 100]");
}

#[test]
fn render_const_zst_tuple_arity_three_recursion_threads_same_const_id() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    });
    let triple = m.types.intern(Ty::Tuple {
        fields: vec![unit, unit, unit],
        layout: UNIT_LAYOUT,
    });
    let id = m.consts.intern(Const::ZST);
    assert_eq!(m.pretty().render_const(id, triple), "((), (), ())");
}

#[test]
fn render_const_zst_tuple_arity_one_emits_trailing_comma() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    });
    let single = m.types.intern(Ty::Tuple {
        fields: vec![unit],
        layout: UNIT_LAYOUT,
    });
    let id = m.consts.intern(Const::ZST);
    assert_eq!(m.pretty().render_const(id, single), "((),)");
}

#[test]
fn render_const_zst_fieldless_struct_renders_def_path() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011::Marker");
    let item = unit_item(&mut m, ItemKind::Struct, rendered);
    let struct_ty = m.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout: UNIT_LAYOUT,
    });
    let id = m.consts.intern(Const::ZST);
    assert_eq!(m.pretty().render_const(id, struct_ty), "pl011::Marker");
}

#[test]
fn render_const_zst_fn_def_renders_value_namespace_path_no_args() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011::init");
    let item = unit_item(&mut m, ItemKind::Function, rendered);
    let ty = m.types.intern(Ty::FnDef(item, vec![]));
    let id = m.consts.intern(Const::ZST);
    assert_eq!(m.pretty().render_const(id, ty), "pl011::init");
}

#[test]
fn render_const_zst_phantom_data_renders_turbofish_value_namespace() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("core::marker::PhantomData");
    let item = unit_item(&mut m, ItemKind::Struct, rendered);
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let struct_ty = m.types.intern(Ty::Struct {
        item,
        args: vec![GenericArg::Type(u32_ty)],
        layout: UNIT_LAYOUT,
    });
    let id = m.consts.intern(Const::ZST);
    assert_eq!(
        m.pretty().render_const(id, struct_ty),
        "core::marker::PhantomData::<u32>"
    );
}

#[test]
fn render_const_zst_union_renders_path_only_no_field_disambiguation() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011::U");
    let item = unit_item(&mut m, ItemKind::Union, rendered);
    let union_ty = m.types.intern(Ty::Union {
        item,
        args: vec![],
        layout: UNIT_LAYOUT,
    });
    let id = m.consts.intern(Const::ZST);
    assert_eq!(m.pretty().render_const(id, union_ty), "pl011::U");
}

#[test]
fn render_const_zst_closure_renders_value_namespace_path() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011::caller::{closure#0}");
    let item = unit_item(&mut m, ItemKind::Function, rendered);
    let ty = m.types.intern(Ty::Closure(item, vec![]));
    let id = m.consts.intern(Const::ZST);
    assert_eq!(
        m.pretty().render_const(id, ty),
        "pl011::caller::{closure#0}"
    );
}

#[test]
fn render_const_zst_single_unit_variant_enum_renders_enum_path_variant() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011::Marker");
    let item = unit_item(&mut m, ItemKind::Enum, rendered);
    let variant_name = m.strings.intern("Only");
    let layout = Layout {
        size: 0,
        align: 1,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Enum {
            shape: EnumShape::Single {
                variant: VariantLayout {
                    name: variant_name,
                    fields: vec![],
                    uninhabited: false,
                    discriminant: 0,
                    tag_value: None,
                },
                scalar: None,
            },
        },
    };
    let enum_ty = m.types.intern(Ty::Enum {
        item,
        args: vec![],
        layout,
    });
    let id = m.consts.intern(Const::ZST);
    assert_eq!(m.pretty().render_const(id, enum_ty), "pl011::Marker::Only");
}

#[test]
fn render_const_zst_struct_with_all_zst_tuple_synthetic_fields_renders_paren_list() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011::TupleHolder");
    let item = unit_item(&mut m, ItemKind::Struct, rendered);
    let unit_ty = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    });
    let name0 = m.strings.intern("_0");
    let name1 = m.strings.intern("_1");
    let layout = Layout {
        size: 0,
        align: 1,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct {
            fields: vec![
                FieldLayout {
                    name: name0,
                    ty: unit_ty,
                    offset: 0,
                },
                FieldLayout {
                    name: name1,
                    ty: unit_ty,
                    offset: 0,
                },
            ],
        },
    };
    let struct_ty = m.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout,
    });
    let id = m.consts.intern(Const::ZST);
    assert_eq!(
        m.pretty().render_const(id, struct_ty),
        "pl011::TupleHolder((), ())"
    );
}

#[test]
fn render_const_zst_struct_with_all_zst_named_fields_renders_brace_list() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011::NamedHolder");
    let item = unit_item(&mut m, ItemKind::Struct, rendered);
    let unit_ty = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    });
    let alpha = m.strings.intern("alpha");
    let beta = m.strings.intern("beta");
    let layout = Layout {
        size: 0,
        align: 1,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct {
            fields: vec![
                FieldLayout {
                    name: alpha,
                    ty: unit_ty,
                    offset: 0,
                },
                FieldLayout {
                    name: beta,
                    ty: unit_ty,
                    offset: 0,
                },
            ],
        },
    };
    let struct_ty = m.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout,
    });
    let id = m.consts.intern(Const::ZST);
    assert_eq!(
        m.pretty().render_const(id, struct_ty),
        "pl011::NamedHolder { alpha: (), beta: () }"
    );
}

// ----- ZST -- panic / cannot-occur rows ---------------------------

#[test]
#[should_panic(expected = "Const::ZST consumed by non-ZST Ty.kind()")]
fn render_const_zst_panics_on_non_zst_consumer_primitive() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let id = m.consts.intern(Const::ZST);
    let _ = m.pretty().render_const(id, u32_ty);
}

#[test]
#[should_panic(expected = "multi-variant Ty::Enum")]
fn render_const_zst_panics_on_multi_variant_enum() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011::Multi");
    let item = unit_item(&mut m, ItemKind::Enum, rendered);
    let variant_name = m.strings.intern("A");
    let variant_b = m.strings.intern("B");
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let layout = Layout {
        size: 1,
        align: 1,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Enum {
            shape: EnumShape::Multiple {
                tag: TagScalar {
                    offset: 0,
                    scalar_ty: u8_ty,
                    valid_range: WrappingRange { start: 0, end: 1 },
                },
                encoding: TagEncoding::Direct,
                variants: vec![
                    VariantLayout {
                        name: variant_name,
                        fields: vec![],
                        uninhabited: false,
                        discriminant: 0,
                        tag_value: Some(0),
                    },
                    VariantLayout {
                        name: variant_b,
                        fields: vec![],
                        uninhabited: false,
                        discriminant: 1,
                        tag_value: Some(1),
                    },
                ],
            },
        },
    };
    let enum_ty = m.types.intern(Ty::Enum {
        item,
        args: vec![],
        layout,
    });
    let id = m.consts.intern(Const::ZST);
    let _ = m.pretty().render_const(id, enum_ty);
}

#[test]
#[should_panic(expected = "zero-variant Ty::Enum")]
fn render_const_zst_panics_on_empty_enum() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let rendered = m.strings.intern("pl011::Void");
    let item = unit_item(&mut m, ItemKind::Enum, rendered);
    let layout = Layout {
        size: 0,
        align: 1,
        pack: None,
        uninhabited: true,
        kind: LayoutKind::Enum {
            shape: EnumShape::Empty,
        },
    };
    let enum_ty = m.types.intern(Ty::Enum {
        item,
        args: vec![],
        layout,
    });
    let id = m.consts.intern(Const::ZST);
    let _ = m.pretty().render_const(id, enum_ty);
}

#[test]
#[should_panic(expected = "Const::ZST consumed by non-ZST Ty.kind()")]
fn render_const_zst_panics_on_never_consumer() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let never_ty = m.types.intern(Ty::Never);
    let id = m.consts.intern(Const::ZST);
    let _ = m.pretty().render_const(id, never_ty);
}
