//! Integration tests for `PrettyPrinter::render_item` per
//! `pretty-printer.md` §9.9 worked-example specimens (P1a-PP-01)
//! and `PrettyPrinter::render_ty` per §9.1 specimens 1 - 25
//! (P1a-PP-02).
//!
//! Each specimen pins the exact textual contract for a row in the
//! §5.3 / §5.4 / §5.5 tables (`render_item`) or in §6.1 - §6.6
//! (`render_ty`); insta snapshots make accidental reformatting a
//! visible event. The §4 / §4.3 Item-header span gating gets a
//! separate set of integration tests in this file.

use insta::assert_snapshot;
use regseq_model::{
    AutoTrait, Body, DefPath, DefPathData, DefPathSegment, FloatKind, FnAbi, FnSig, GenericArg,
    InlineHint, Instance, InstanceKind, IntKind, Item, ItemId, ItemKind, Linkage, Mutability,
    PrettyPrintOptions, PrettyPrinter, Provenance, RegseqModel, Safety, SourceFile, SourceFileId,
    Span, StableCrateId, Ty, UintKind, UNIT_LAYOUT,
};
use regseq_model::{FunctionInstanceData, FunctionInstanceFlavor};

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

// ------------------------------------------------------------------
// §9.1 render_ty specimens (pretty-printer.md:1191-1318).
// Each insta snapshot pins the exact textual output of one of the
// 25 worked-example specimens.
// ------------------------------------------------------------------

#[test]
fn render_ty_specimen_01_bool() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = m.types.intern(Ty::Bool);
    assert_snapshot!(m.pretty().render_ty(id), @"bool");
}

#[test]
fn render_ty_specimen_02_int_i32() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = m.types.intern(Ty::Int(IntKind::I32));
    assert_snapshot!(m.pretty().render_ty(id), @"i32");
}

#[test]
fn render_ty_specimen_03_uint_u32() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = m.types.intern(Ty::Uint(UintKind::U32));
    assert_snapshot!(m.pretty().render_ty(id), @"u32");
}

#[test]
fn render_ty_specimen_04_float_f64() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = m.types.intern(Ty::Float(FloatKind::F64));
    assert_snapshot!(m.pretty().render_ty(id), @"f64");
}

#[test]
fn render_ty_specimen_05_char() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = m.types.intern(Ty::Char);
    assert_snapshot!(m.pretty().render_ty(id), @"char");
}

#[test]
fn render_ty_specimen_06_str() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = m.types.intern(Ty::Str);
    assert_snapshot!(m.pretty().render_ty(id), @"str");
}

#[test]
fn render_ty_specimen_07_never() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = m.types.intern(Ty::Never);
    assert_snapshot!(m.pretty().render_ty(id), @"!");
}

#[test]
fn render_ty_specimen_08_array_u8_32() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let id = m.types.intern(Ty::Array(u8_ty, 32));
    assert_snapshot!(m.pretty().render_ty(id), @"[u8; 32]");
}

#[test]
fn render_ty_specimen_09_slice_u8() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let id = m.types.intern(Ty::Slice(u8_ty));
    assert_snapshot!(m.pretty().render_ty(id), @"[u8]");
}

#[test]
fn render_ty_specimen_10_ref_shared_i32() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let id = m.types.intern(Ty::Ref(i32_ty, Provenance::SharedBorrow));
    assert_snapshot!(m.pretty().render_ty(id), @"&i32");
}

#[test]
fn render_ty_specimen_11_ref_unique_i32() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let id = m.types.intern(Ty::Ref(i32_ty, Provenance::UniqueBorrow));
    assert_snapshot!(m.pretty().render_ty(id), @"&mut i32");
}

#[test]
fn render_ty_specimen_12_ptr_const_u8() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let id = m.types.intern(Ty::Ptr(u8_ty, Mutability::Not));
    assert_snapshot!(m.pretty().render_ty(id), @"*const u8");
}

#[test]
fn render_ty_specimen_13_ptr_mut_u8() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let id = m.types.intern(Ty::Ptr(u8_ty, Mutability::Mut));
    assert_snapshot!(m.pretty().render_ty(id), @"*mut u8");
}

#[test]
fn render_ty_specimen_14_tuple_unit() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let id = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    });
    assert_snapshot!(m.pretty().render_ty(id), @"()");
}

#[test]
fn render_ty_specimen_15_tuple_arity_one_i32() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let id = m.types.intern(Ty::Tuple {
        fields: vec![i32_ty],
        layout: UNIT_LAYOUT,
    });
    assert_snapshot!(m.pretty().render_ty(id), @"(i32,)");
}

#[test]
fn render_ty_specimen_16_tuple_arity_two_i32_bool() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let bool_ty = m.types.intern(Ty::Bool);
    let id = m.types.intern(Ty::Tuple {
        fields: vec![i32_ty, bool_ty],
        layout: UNIT_LAYOUT,
    });
    assert_snapshot!(m.pretty().render_ty(id), @"(i32, bool)");
}

#[test]
fn render_ty_specimen_17_struct_pl011() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_item(&mut m, ItemKind::Struct, "pl011::PL011");
    let id = m.types.intern(Ty::Struct {
        item,
        args: vec![],
        layout: UNIT_LAYOUT,
    });
    assert_snapshot!(m.pretty().render_ty(id), @"pl011::PL011");
}

#[test]
fn render_ty_specimen_18_enum_option_u32() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_item(&mut m, ItemKind::Enum, "core::option::Option");
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let id = m.types.intern(Ty::Enum {
        item,
        args: vec![GenericArg::Type(u32_ty)],
        layout: UNIT_LAYOUT,
    });
    assert_snapshot!(m.pretty().render_ty(id), @"core::option::Option<u32>");
}

#[test]
fn render_ty_specimen_19_union_my_union() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let item = push_item(&mut m, ItemKind::Union, "pl011::regs::MyUnion");
    let id = m.types.intern(Ty::Union {
        item,
        args: vec![],
        layout: UNIT_LAYOUT,
    });
    assert_snapshot!(m.pretty().render_ty(id), @"pl011::regs::MyUnion");
}

#[test]
fn render_ty_specimen_20_fn_def_pl011_init() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let pl011_item = push_item(&mut m, ItemKind::Struct, "pl011::PL011");
    let init_item = push_item(&mut m, ItemKind::Function, "pl011::init");
    let pl011_ty = m.types.intern(Ty::Struct {
        item: pl011_item,
        args: vec![],
        layout: UNIT_LAYOUT,
    });
    let ptr_pl011 = m.types.intern(Ty::Ptr(pl011_ty, Mutability::Mut));
    let bool_ty = m.types.intern(Ty::Bool);
    let sig = m.sigs.intern(FnSig {
        inputs: vec![ptr_pl011],
        output: bool_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let emission_name = m.strings.intern("pl011_init");
    m.instances.push(Instance {
        item: init_item,
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
    let id = m.types.intern(Ty::FnDef(init_item, vec![]));
    assert_snapshot!(
        m.pretty().render_ty(id),
        @"fn(*mut pl011::PL011) -> bool {pl011::init}"
    );
}

#[test]
fn render_ty_specimen_21_fn_ptr_safe_rust() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let sig = m.sigs.intern(FnSig {
        inputs: vec![i32_ty],
        output: u32_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let id = m.types.intern(Ty::FnPtr(sig));
    assert_snapshot!(m.pretty().render_ty(id), @"fn(i32) -> u32");
}

#[test]
fn render_ty_specimen_22_fn_ptr_unsafe_extern_c() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let ptr_u8 = m.types.intern(Ty::Ptr(u8_ty, Mutability::Mut));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let sig = m.sigs.intern(FnSig {
        inputs: vec![ptr_u8],
        output: i32_ty,
        abi: FnAbi::C,
        c_variadic: false,
        safety: Safety::Unsafe,
    });
    let id = m.types.intern(Ty::FnPtr(sig));
    assert_snapshot!(
        m.pretty().render_ty(id),
        @r###"unsafe extern "C" fn(*mut u8) -> i32"###
    );
}

#[test]
fn render_ty_specimen_23_dynamic_with_auto_traits() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let vtable_item = push_item(
        &mut m,
        ItemKind::Struct,
        "<regseq_synth>::vtable::Reader::for::<std::fs::File>",
    );
    let vtable_ty = m.types.intern(Ty::Struct {
        item: vtable_item,
        args: vec![],
        layout: UNIT_LAYOUT,
    });
    let id = m.types.intern(Ty::Dynamic {
        auto_traits: vec![AutoTrait::Send, AutoTrait::Sync],
        vtable_ty,
    });
    assert_snapshot!(
        m.pretty().render_ty(id),
        @"dyn <regseq_synth>::vtable::Reader::for::<std::fs::File> + Send + Sync"
    );
}

#[test]
fn render_ty_specimen_24_dynamic_without_auto_traits() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let vtable_item = push_item(
        &mut m,
        ItemKind::Struct,
        "<regseq_synth>::vtable::Foo::for::<Bar>",
    );
    let vtable_ty = m.types.intern(Ty::Struct {
        item: vtable_item,
        args: vec![],
        layout: UNIT_LAYOUT,
    });
    let id = m.types.intern(Ty::Dynamic {
        auto_traits: vec![],
        vtable_ty,
    });
    assert_snapshot!(
        m.pretty().render_ty(id),
        @"dyn <regseq_synth>::vtable::Foo::for::<Bar>"
    );
}

#[test]
fn render_ty_specimen_25_closure_with_generic_args() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let closure_item = push_item(&mut m, ItemKind::Function, "pl011::init::{closure#0}");
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let id = m
        .types
        .intern(Ty::Closure(closure_item, vec![GenericArg::Type(i32_ty)]));
    assert_snapshot!(m.pretty().render_ty(id), @"pl011::init::{closure#0}<i32>");
}
