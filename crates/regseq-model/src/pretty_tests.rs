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
