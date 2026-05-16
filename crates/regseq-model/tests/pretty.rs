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
    AggregateKind, AssertKind, AutoTrait, BasicBlock, BinOp, BlockId, Body, CallCallee, CastKind,
    ConstId, CopyNonOverlapping, DefPath, DefPathData, DefPathSegment, EnumShape, FieldIdx,
    FieldLayout, FloatKind, FnAbi, FnSig, GenericArg, InlineHint, Instance, InstanceKind, IntKind,
    Item, ItemId, ItemKind, Layout, LayoutKind, Linkage, LocalDecl, LocalId, Mutability,
    NonDivergingIntrinsic, Operand, Place, PlaceElem, PointerCoercion, PrettyPrintOptions,
    PrettyPrinter, Provenance, RegseqModel, Rvalue, Safety, SourceFile, SourceFileId, Span,
    StableCrateId, Statement, StatementKind, StringId, SwitchTargets, TagEncoding, TagScalar,
    Terminator, TerminatorKind, Ty, TyId, UintKind, UnOp, VariantIdx, VariantLayout, WrappingRange,
    UNIT_LAYOUT,
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

// ------------------------------------------------------------------
// §9.2 - §9.8 body-level specimens (P1a-PP-03 / regseq-model-88).
//
// Specimens that inherently include `Operand::Constant` (and so
// hit `render_const`, which is unimplemented in this task per Non-
// Goals) are marked `#[should_panic]` so the dispatch shape is
// still verified. Once P1a-PP-04a/b lands, those tests will be
// migrated to full snapshot form.
// ------------------------------------------------------------------

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

fn intern_unit(m: &mut RegseqModel) -> TyId {
    m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: UNIT_LAYOUT,
    })
}

fn intern_named_struct(m: &mut RegseqModel, rendered: &str, fields: Vec<(&str, TyId)>) -> TyId {
    let item = push_item(m, ItemKind::Struct, rendered);
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

fn intern_two_variant_enum(
    m: &mut RegseqModel,
    rendered: &str,
    args: Vec<GenericArg>,
    variants: Vec<(&str, Vec<(&str, TyId)>)>,
) -> TyId {
    let item = push_item(m, ItemKind::Enum, rendered);
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
    let variants_len = layouts.len() as u128;
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
                    tag: TagScalar {
                        offset: 0,
                        scalar_ty: u32_ty,
                        valid_range: WrappingRange {
                            start: 0,
                            end: variants_len,
                        },
                    },
                    encoding: TagEncoding::Direct,
                    variants: layouts,
                },
            },
        },
    })
}

// ----- §9.2 render_place specimen ----------------------------------

#[test]
fn render_place_specimen_26_long_projection_chain() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let usize_ty = m.types.intern(Ty::Uint(UintKind::Usize));

    let pl011_item = push_item(&mut m, ItemKind::Struct, "pl011::PL011RegBlock");
    let mask_name = m.strings.intern("mask");
    let dr_name = m.strings.intern("dr");
    let fr_name = m.strings.intern("fr");
    let pl011_ty = m.types.intern(Ty::Struct {
        item: pl011_item,
        args: vec![],
        layout: Layout {
            size: 12,
            align: 4,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Struct {
                fields: vec![
                    FieldLayout {
                        name: dr_name,
                        ty: u32_ty,
                        offset: 0,
                    },
                    FieldLayout {
                        name: fr_name,
                        ty: u32_ty,
                        offset: 4,
                    },
                    FieldLayout {
                        name: mask_name,
                        ty: u32_ty,
                        offset: 8,
                    },
                ],
            },
        },
    });
    let ptr_pl011 = m.types.intern(Ty::Ptr(pl011_ty, Mutability::Mut));

    let mut body = Body::default();
    // _0..=_2 filler, _3 = *mut PL011RegBlock, _4..=_7 fillers.
    body.push_local(dummy_local(usize_ty));
    body.push_local(dummy_local(usize_ty));
    body.push_local(dummy_local(usize_ty));
    body.push_local(dummy_local(ptr_pl011));
    push_locals(&mut body, 4, usize_ty);

    let place = Place {
        local: LocalId::from_u32(3),
        projection: Box::new([
            PlaceElem::Deref,
            PlaceElem::Field(FieldIdx::from_u32(2), u32_ty),
            PlaceElem::Index(LocalId::from_u32(7)),
        ]),
    };
    assert_snapshot!(
        m.pretty().render_place(&place, &body),
        @"((*_3).mask: u32)[_7]"
    );
}

// ----- §9.3 render_operand specimens -------------------------------

#[test]
fn render_operand_specimen_27_copy() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, 3, u32_ty);
    let op = Operand::Copy(Place::from(LocalId::from_u32(2)));
    assert_snapshot!(m.pretty().render_operand(&op, &body, u32_ty), @"copy _2");
}

#[test]
fn render_operand_specimen_28_move() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, 5, u32_ty);
    let op = Operand::Move(Place::from(LocalId::from_u32(4)));
    assert_snapshot!(m.pretty().render_operand(&op, &body, u32_ty), @"move _4");
}

#[test]
#[should_panic(expected = "stage 14 fills render_const")]
fn render_operand_specimen_29_constant_dispatches_to_render_const() {
    // Specimen 29 (Operand::Constant) inherently routes to
    // `render_const`, which is the stage-14 / P1a-PP-04a/b owner.
    // The `#[should_panic]` verifies the dispatch reaches the stub
    // with the correct pair-form call shape `(id, expected_ty)`.
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let body = Body::default();
    let op = Operand::Constant(ConstId::ZERO);
    let _ = m.pretty().render_operand(&op, &body, i32_ty);
}

// ----- §9.4 render_rvalue specimens --------------------------------

#[test]
fn render_rvalue_specimen_30_use_copy() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, 4, u32_ty);
    let rv = Rvalue::Use(Operand::Copy(Place::from(LocalId::from_u32(3))));
    assert_snapshot!(m.pretty().render_rvalue(&rv, &body, u32_ty), @"copy _3");
}

#[test]
#[should_panic(expected = "stage 14 fills render_const")]
fn render_rvalue_specimen_31_repeat_constant_dispatches_to_render_const() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let array_ty = m.types.intern(Ty::Array(u8_ty, 16));
    let body = Body::default();
    let rv = Rvalue::Repeat(Operand::Constant(ConstId::ZERO), 16);
    let _ = m.pretty().render_rvalue(&rv, &body, array_ty);
}

#[test]
fn render_rvalue_specimen_32_ref_shared() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let ref_ty = m.types.intern(Ty::Ref(u32_ty, Provenance::SharedBorrow));
    let mut body = Body::default();
    push_locals(&mut body, 3, u32_ty);
    let rv = Rvalue::Ref(Provenance::SharedBorrow, Place::from(LocalId::from_u32(2)));
    assert_snapshot!(m.pretty().render_rvalue(&rv, &body, ref_ty), @"&_2");
}

#[test]
fn render_rvalue_specimen_33_ref_unique() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let ref_ty = m.types.intern(Ty::Ref(u32_ty, Provenance::UniqueBorrow));
    let mut body = Body::default();
    push_locals(&mut body, 3, u32_ty);
    let rv = Rvalue::Ref(Provenance::UniqueBorrow, Place::from(LocalId::from_u32(2)));
    assert_snapshot!(m.pretty().render_rvalue(&rv, &body, ref_ty), @"&mut _2");
}

#[test]
fn render_rvalue_specimen_34_raw_ptr_const() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let ptr_ty = m.types.intern(Ty::Ptr(u32_ty, Mutability::Not));
    let mut body = Body::default();
    push_locals(&mut body, 6, u32_ty);
    let rv = Rvalue::RawPtr(Mutability::Not, Place::from(LocalId::from_u32(5)));
    assert_snapshot!(m.pretty().render_rvalue(&rv, &body, ptr_ty), @"&raw const _5");
}

#[test]
fn render_rvalue_specimen_35_raw_ptr_mut() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let ptr_ty = m.types.intern(Ty::Ptr(u32_ty, Mutability::Mut));
    let mut body = Body::default();
    push_locals(&mut body, 6, u32_ty);
    let rv = Rvalue::RawPtr(Mutability::Mut, Place::from(LocalId::from_u32(5)));
    assert_snapshot!(m.pretty().render_rvalue(&rv, &body, ptr_ty), @"&raw mut _5");
}

#[test]
fn render_rvalue_specimen_36_cast_unambiguous() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let i64_ty = m.types.intern(Ty::Int(IntKind::I64));
    let mut body = Body::default();
    push_locals(&mut body, 4, i32_ty);
    let rv = Rvalue::Cast(
        CastKind::IntToInt,
        Operand::Copy(Place::from(LocalId::from_u32(3))),
        i64_ty,
    );
    assert_snapshot!(m.pretty().render_rvalue(&rv, &body, i64_ty), @"copy _3 as i64");
}

#[test]
fn render_rvalue_specimen_37_cast_pointer_coercion_ambiguous() {
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
    let fn_ptr_ty = m.types.intern(Ty::FnPtr(sig));
    let mut body = Body::default();
    push_locals(&mut body, 3, i32_ty);
    body.push_local(dummy_local(fn_ptr_ty));
    let rv = Rvalue::Cast(
        CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer),
        Operand::Move(Place::from(LocalId::from_u32(3))),
        fn_ptr_ty,
    );
    assert_snapshot!(
        m.pretty().render_rvalue(&rv, &body, fn_ptr_ty),
        @"move _3 as fn(i32) -> u32 (PointerCoercion(ReifyFnPointer))"
    );
}

#[test]
fn render_rvalue_specimen_38_binary_op() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, 6, u32_ty);
    let rv = Rvalue::BinaryOp(
        BinOp::Add,
        Operand::Copy(Place::from(LocalId::from_u32(3))),
        Operand::Copy(Place::from(LocalId::from_u32(4))),
    );
    assert_snapshot!(
        m.pretty().render_rvalue(&rv, &body, u32_ty),
        @"copy _3 + copy _4"
    );
}

#[test]
fn render_rvalue_specimen_39_unary_op_neg() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let mut body = Body::default();
    push_locals(&mut body, 5, i32_ty);
    let rv = Rvalue::UnaryOp(UnOp::Neg, Operand::Copy(Place::from(LocalId::from_u32(3))));
    assert_snapshot!(m.pretty().render_rvalue(&rv, &body, i32_ty), @"-copy _3");
}

#[test]
fn render_rvalue_specimen_40_discriminant() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, 6, u32_ty);
    let rv = Rvalue::Discriminant(Place::from(LocalId::from_u32(5)));
    assert_snapshot!(
        m.pretty().render_rvalue(&rv, &body, u32_ty),
        @"discriminant(_5)"
    );
}

#[test]
#[should_panic(expected = "stage 14 fills render_const")]
fn render_rvalue_specimen_41_aggregate_array_constants_dispatch() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let array_ty = m.types.intern(Ty::Array(u8_ty, 3));
    let body = Body::default();
    let rv = Rvalue::Aggregate(
        AggregateKind::Array(u8_ty),
        Box::new([
            Operand::Constant(ConstId::ZERO),
            Operand::Constant(ConstId::ZERO),
            Operand::Constant(ConstId::ZERO),
        ]),
    );
    let _ = m.pretty().render_rvalue(&rv, &body, array_ty);
}

#[test]
#[should_panic(expected = "stage 14 fills render_const")]
fn render_rvalue_specimen_42_aggregate_tuple_arity_two_constants_dispatch() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let bool_ty = m.types.intern(Ty::Bool);
    let tuple_ty = m.types.intern(Ty::Tuple {
        fields: vec![i32_ty, bool_ty],
        layout: UNIT_LAYOUT,
    });
    let body = Body::default();
    let rv = Rvalue::Aggregate(
        AggregateKind::Tuple(tuple_ty),
        Box::new([
            Operand::Constant(ConstId::ZERO),
            Operand::Constant(ConstId::ZERO),
        ]),
    );
    let _ = m.pretty().render_rvalue(&rv, &body, tuple_ty);
}

#[test]
#[should_panic(expected = "stage 14 fills render_const")]
fn render_rvalue_specimen_43_aggregate_struct_named_fields_dispatch() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let struct_ty = intern_named_struct(&mut m, "geom::Point", vec![("x", i32_ty), ("y", i32_ty)]);
    let body = Body::default();
    let rv = Rvalue::Aggregate(
        AggregateKind::Struct(struct_ty),
        Box::new([
            Operand::Constant(ConstId::ZERO),
            Operand::Constant(ConstId::ZERO),
        ]),
    );
    let _ = m.pretty().render_rvalue(&rv, &body, struct_ty);
}

#[test]
#[should_panic(expected = "stage 14 fills render_const")]
fn render_rvalue_specimen_44_aggregate_enum_tuple_variant_dispatch() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let enum_ty = intern_two_variant_enum(
        &mut m,
        "core::option::Option",
        vec![GenericArg::Type(u32_ty)],
        vec![("None", vec![]), ("Some", vec![("0", u32_ty)])],
    );
    let body = Body::default();
    let rv = Rvalue::Aggregate(
        AggregateKind::Enum(enum_ty, VariantIdx::from_u32(1)),
        Box::new([Operand::Constant(ConstId::ZERO)]),
    );
    let _ = m.pretty().render_rvalue(&rv, &body, enum_ty);
}

#[test]
fn render_rvalue_specimen_45_aggregate_enum_c_style() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let enum_ty = intern_two_variant_enum(
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
    assert_snapshot!(m.pretty().render_rvalue(&rv, &body, enum_ty), @"pl011::Color::Red");
}

#[test]
#[should_panic(expected = "stage 14 fills render_const")]
fn render_rvalue_specimen_46_aggregate_union_dispatch() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let union_item = push_item(&mut m, ItemKind::Union, "pl011::regs::MyUnion");
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
    let body = Body::default();
    let rv = Rvalue::Aggregate(
        AggregateKind::Union(union_ty, FieldIdx::from_u32(1)),
        Box::new([Operand::Constant(ConstId::ZERO)]),
    );
    let _ = m.pretty().render_rvalue(&rv, &body, union_ty);
}

#[test]
fn render_rvalue_specimen_47_aggregate_closure_with_move_capture() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let closure_item = push_item(&mut m, ItemKind::Function, "pl011::init::{closure#0}");
    let count_name = m.strings.intern("count");
    // Parallel captures struct shares (item, args) identity per spec §13.1.
    let _captures_ty = m.types.intern(Ty::Struct {
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
    push_locals(&mut body, 4, u32_ty);
    let rv = Rvalue::Aggregate(
        AggregateKind::Closure(closure_ty),
        Box::new([Operand::Move(Place::from(LocalId::from_u32(3)))]),
    );
    assert_snapshot!(
        m.pretty().render_rvalue(&rv, &body, closure_ty),
        @"pl011::init::{closure#0} { count: move _3 }"
    );
}

#[test]
fn render_rvalue_specimen_47b_aggregate_raw_ptr_thin() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let unit_t = intern_unit(&mut m);
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
    assert_snapshot!(
        m.pretty().render_rvalue(&rv, &body, ptr_u8_mut),
        @"(copy _3, copy _4) as *mut u8"
    );
}

#[test]
fn render_rvalue_specimen_47c_aggregate_raw_ptr_fat() {
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
    assert_snapshot!(
        m.pretty().render_rvalue(&rv, &body, ptr_slice_const),
        @"(copy _3, copy _4) as *const [u8]"
    );
}

// ----- §9.5 render_statement specimens -----------------------------

#[test]
#[should_panic(expected = "stage 14 fills render_const")]
fn render_statement_specimen_48_assign_constant_dispatches() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let mut body = Body::default();
    push_locals(&mut body, 4, i32_ty);
    let stmt = Statement {
        kind: StatementKind::Assign(
            Place::from(LocalId::from_u32(3)),
            Rvalue::Use(Operand::Constant(ConstId::ZERO)),
        ),
        span: Span::dummy(),
    };
    let _ = m.pretty().render_statement(&stmt, &body);
}

#[test]
fn render_statement_specimen_49_set_discriminant() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let enum_ty = intern_two_variant_enum(
        &mut m,
        "core::option::Option",
        vec![GenericArg::Type(i32_ty)],
        vec![("None", vec![]), ("Some", vec![("0", i32_ty)])],
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
    assert_snapshot!(
        m.pretty().render_statement(&stmt, &body),
        @"discriminant(_3) = Some;"
    );
}

#[test]
fn render_statement_specimen_50_intrinsic_assume() {
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
    assert_snapshot!(
        m.pretty().render_statement(&stmt, &body),
        @"assume(copy _5);"
    );
}

#[test]
#[should_panic(expected = "stage 14 fills render_const")]
fn render_statement_specimen_51_intrinsic_copy_non_overlapping_count_const_dispatches() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let ptr_const = m.types.intern(Ty::Ptr(u8_ty, Mutability::Not));
    let ptr_mut = m.types.intern(Ty::Ptr(u8_ty, Mutability::Mut));
    let mut body = Body::default();
    body.push_local(dummy_local(ptr_const));
    body.push_local(dummy_local(ptr_const));
    body.push_local(dummy_local(ptr_const));
    body.push_local(dummy_local(ptr_const));
    body.push_local(dummy_local(ptr_mut));
    let stmt = Statement {
        kind: StatementKind::Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(
            CopyNonOverlapping {
                src: Operand::Move(Place::from(LocalId::from_u32(3))),
                dst: Operand::Move(Place::from(LocalId::from_u32(4))),
                count: Operand::Constant(ConstId::ZERO),
            },
        )),
        span: Span::dummy(),
    };
    let _ = m.pretty().render_statement(&stmt, &body);
}

// ----- §9.6 render_terminator specimens ----------------------------

#[test]
fn render_terminator_specimen_52_goto() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let body = Body::default();
    let term = Terminator {
        kind: TerminatorKind::Goto(BlockId::from_u32(3)),
        span: Span::dummy(),
    };
    assert_snapshot!(m.pretty().render_terminator(&term, &body), @"goto -> [bb3]");
}

#[test]
fn render_terminator_specimen_53_switch_int() {
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
    assert_snapshot!(
        m.pretty().render_terminator(&term, &body),
        @"switchInt(copy _3) -> [0: bb1, 1: bb2, otherwise: bb3]"
    );
}

#[test]
fn render_terminator_specimen_54_return() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let body = Body::default();
    let term = Terminator {
        kind: TerminatorKind::Return,
        span: Span::dummy(),
    };
    assert_snapshot!(m.pretty().render_terminator(&term, &body), @"return -> []");
}

#[test]
fn render_terminator_specimen_55_unreachable() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let body = Body::default();
    let term = Terminator {
        kind: TerminatorKind::Unreachable,
        span: Span::dummy(),
    };
    assert_snapshot!(
        m.pretty().render_terminator(&term, &body),
        @"unreachable -> []"
    );
}

fn push_write_byte_u8_instance(m: &mut RegseqModel) -> (regseq_model::InstanceId, TyId) {
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let bool_ty = m.types.intern(Ty::Bool);
    let item = push_item(m, ItemKind::Function, "pl011::write_byte");
    let sig = m.sigs.intern(FnSig {
        inputs: vec![u8_ty, u8_ty],
        output: bool_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let emission = m.strings.intern("pl011_write_byte");
    let id = m.instances.push(Instance {
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
    (id, u8_ty)
}

#[test]
fn render_terminator_specimen_56_call_direct() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let bool_ty = m.types.intern(Ty::Bool);
    let (id, u8_ty) = push_write_byte_u8_instance(&mut m);
    let mut body = Body::default();
    body.push_local(dummy_local(bool_ty));
    body.push_local(dummy_local(u8_ty));
    body.push_local(dummy_local(u8_ty));
    body.push_local(dummy_local(u8_ty));
    body.push_local(dummy_local(u8_ty));
    let term = Terminator {
        kind: TerminatorKind::Call {
            callee: CallCallee::Direct(id),
            args: Box::new([
                Operand::Move(Place::from(LocalId::from_u32(3))),
                Operand::Copy(Place::from(LocalId::from_u32(4))),
            ]),
            destination: Place::from(LocalId::from_u32(0)),
            target: Some(BlockId::from_u32(2)),
        },
        span: Span::dummy(),
    };
    assert_snapshot!(
        m.pretty().render_terminator(&term, &body),
        @"_0 = pl011::write_byte<u8>(move _3, copy _4) -> [return: bb2]"
    );
}

#[test]
fn render_terminator_specimen_57_call_indirect() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let bool_ty = m.types.intern(Ty::Bool);
    let sig = m.sigs.intern(FnSig {
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
    assert_snapshot!(
        m.pretty().render_terminator(&term, &body),
        @"_0 = copy _5(move _3) -> [return: bb2]"
    );
}

#[test]
fn render_terminator_specimen_58_assert_bounds_check() {
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
    assert_snapshot!(
        m.pretty().render_terminator(&term, &body),
        @"assert(true, copy _3, index out of bounds: the length is copy _4 but the index is copy _5) -> [success: bb2]"
    );
}

#[test]
fn render_call_callee_specimen_58a_direct_standalone() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let (id, _) = push_write_byte_u8_instance(&mut m);
    let body = Body::default();
    assert_snapshot!(
        m.pretty().render_call_callee(&CallCallee::Direct(id), &body),
        @"pl011::write_byte<u8>"
    );
}

#[test]
fn render_call_callee_specimen_58b_indirect_standalone() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u8_ty = m.types.intern(Ty::Uint(UintKind::U8));
    let mut body = Body::default();
    push_locals(&mut body, 6, u8_ty);
    let callee = CallCallee::Indirect(Operand::Copy(Place::from(LocalId::from_u32(5))));
    assert_snapshot!(m.pretty().render_call_callee(&callee, &body), @"copy _5");
}

#[test]
fn render_switch_targets_specimen_58c_standalone() {
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
    assert_snapshot!(
        m.pretty().render_switch_targets(&targets, &body),
        @"0: bb1, 1: bb2, 5: bb7, otherwise: bb3"
    );
}

// ----- §9.7 render_assert_kind specimens ---------------------------

fn assert_kind_body(m: &mut RegseqModel, head_ty: TyId, count: usize) -> Body {
    let mut body = Body::default();
    push_locals(&mut body, count, head_ty);
    body
}

#[test]
fn render_assert_kind_specimen_59_bounds_check() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let usize_ty = m.types.intern(Ty::Uint(UintKind::Usize));
    let body = assert_kind_body(&mut m, usize_ty, 6);
    let kind = AssertKind::BoundsCheck {
        len: Operand::Copy(Place::from(LocalId::from_u32(4))),
        index: Operand::Copy(Place::from(LocalId::from_u32(5))),
    };
    assert_snapshot!(
        m.pretty().render_assert_kind(&kind, &body),
        @"index out of bounds: the length is copy _4 but the index is copy _5"
    );
}

#[test]
fn render_assert_kind_specimen_60_overflow_add() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let body = assert_kind_body(&mut m, i32_ty, 6);
    let kind = AssertKind::Overflow(
        BinOp::Add,
        Operand::Copy(Place::from(LocalId::from_u32(4))),
        Operand::Copy(Place::from(LocalId::from_u32(5))),
    );
    assert_snapshot!(
        m.pretty().render_assert_kind(&kind, &body),
        @"attempt to compute `copy _4 + copy _5`, which would overflow"
    );
}

#[test]
fn render_assert_kind_specimen_61_overflow_shl() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let mut body = Body::default();
    push_locals(&mut body, 5, i32_ty);
    body.push_local(dummy_local(u32_ty));
    let kind = AssertKind::Overflow(
        BinOp::Shl,
        Operand::Copy(Place::from(LocalId::from_u32(4))),
        Operand::Copy(Place::from(LocalId::from_u32(5))),
    );
    assert_snapshot!(
        m.pretty().render_assert_kind(&kind, &body),
        @"attempt to shift left by `copy _5`, which would overflow"
    );
}

#[test]
fn render_assert_kind_specimen_62_overflow_neg() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let body = assert_kind_body(&mut m, i32_ty, 5);
    let kind = AssertKind::OverflowNeg(Operand::Copy(Place::from(LocalId::from_u32(4))));
    assert_snapshot!(
        m.pretty().render_assert_kind(&kind, &body),
        @"attempt to negate `copy _4`, which would overflow"
    );
}

#[test]
fn render_assert_kind_specimen_63_division_by_zero() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let body = assert_kind_body(&mut m, i32_ty, 5);
    let kind = AssertKind::DivisionByZero(Operand::Copy(Place::from(LocalId::from_u32(4))));
    assert_snapshot!(
        m.pretty().render_assert_kind(&kind, &body),
        @"attempt to divide `copy _4` by zero"
    );
}

#[test]
fn render_assert_kind_specimen_64_remainder_by_zero() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let body = assert_kind_body(&mut m, i32_ty, 5);
    let kind = AssertKind::RemainderByZero(Operand::Copy(Place::from(LocalId::from_u32(4))));
    assert_snapshot!(
        m.pretty().render_assert_kind(&kind, &body),
        @"attempt to calculate the remainder of `copy _4` with a divisor of zero"
    );
}

#[test]
fn render_assert_kind_specimen_65_misaligned_pointer_dereference() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let usize_ty = m.types.intern(Ty::Uint(UintKind::Usize));
    let body = assert_kind_body(&mut m, usize_ty, 6);
    let kind = AssertKind::MisalignedPointerDereference {
        required: Operand::Copy(Place::from(LocalId::from_u32(4))),
        found: Operand::Copy(Place::from(LocalId::from_u32(5))),
    };
    assert_snapshot!(
        m.pretty().render_assert_kind(&kind, &body),
        @"misaligned pointer dereference: address must be a multiple of copy _4 but is copy _5"
    );
}

#[test]
fn render_assert_kind_specimen_66_null_pointer_dereference() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let body = Body::default();
    assert_snapshot!(
        m.pretty().render_assert_kind(&AssertKind::NullPointerDereference, &body),
        @"null pointer dereference occurred"
    );
}

#[test]
fn render_assert_kind_specimen_66b_invalid_enum_construction() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let u32_ty = m.types.intern(Ty::Uint(UintKind::U32));
    let body = assert_kind_body(&mut m, u32_ty, 5);
    let kind =
        AssertKind::InvalidEnumConstruction(Operand::Copy(Place::from(LocalId::from_u32(4))));
    assert_snapshot!(
        m.pretty().render_assert_kind(&kind, &body),
        @"invalid enum discriminant value: copy _4"
    );
}

// ----- §9.8 render_basic_block specimens ---------------------------

#[test]
#[should_panic(expected = "stage 14 fills render_const")]
fn render_basic_block_specimen_67_multi_statement_const_dispatches() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let mut body = Body::default();
    push_locals(&mut body, 6, i32_ty);
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([
            Statement {
                kind: StatementKind::Assign(
                    Place::from(LocalId::from_u32(3)),
                    Rvalue::Use(Operand::Constant(ConstId::ZERO)),
                ),
                span: Span::dummy(),
            },
            Statement {
                kind: StatementKind::Assign(
                    Place::from(LocalId::from_u32(4)),
                    Rvalue::BinaryOp(
                        BinOp::Add,
                        Operand::Copy(Place::from(LocalId::from_u32(3))),
                        Operand::Constant(ConstId::ZERO),
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
    let _ = m.pretty().render_basic_block(block, &body);
}

#[test]
fn render_basic_block_specimen_68_empty_statements_block() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let mut body = Body::default();
    // Push a filler block at index 0 so the rendered block lands at
    // BlockId(1), matching the specimen.
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
    assert_snapshot!(
        m.pretty().render_basic_block(block, &body),
        @r###"
        bb1: {
            goto -> [bb2];
        }
        "###
    );
}

// ----- §4 body-level span gating integration ----------------------

fn body_with_file(m: &mut RegseqModel, ty: TyId, count: usize) -> (Body, SourceFileId) {
    let path = m.strings.intern("pl011.rs");
    let file_id = m.source_files.push(SourceFile {
        path,
        content: Some("let x = 1;\nlet y = 2;\n".into()),
        line_starts: vec![0, 11],
    });
    let mut body = Body::default();
    push_locals(&mut body, count, ty);
    (body, file_id)
}

#[test]
fn body_level_span_gating_default_off_for_statement_and_terminator() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let (body, file_id) = body_with_file(&mut m, i32_ty, 4);
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
    let term = Terminator {
        kind: TerminatorKind::Return,
        span: Span {
            file: file_id,
            lo: 0,
            hi: 5,
        },
    };
    let p = m.pretty();
    assert_eq!(p.render_statement(&stmt, &body), "_3 = copy _2;");
    assert_eq!(p.render_terminator(&term, &body), "return -> []");
}

#[test]
fn body_level_span_gating_include_spans_on_non_synthesized() {
    let mut m = RegseqModel::with_primary("pl011-spec", StableCrateId(1));
    let i32_ty = m.types.intern(Ty::Int(IntKind::I32));
    let (body, file_id) = body_with_file(&mut m, i32_ty, 4);
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
    let term = Terminator {
        kind: TerminatorKind::Return,
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
    let p = PrettyPrinter::with_options(&m, opts);
    let stmt_head = "_3 = copy _2;";
    let term_head = "return -> []";
    assert_eq!(
        p.render_statement(&stmt, &body),
        format!("{stmt_head:<40} // pl011.rs:1:1: 1:6"),
    );
    assert_eq!(
        p.render_terminator(&term, &body),
        format!("{term_head:<40} // pl011.rs:1:1: 1:6"),
    );
}

#[test]
fn body_level_span_gating_include_spans_suppressed_when_synthesized() {
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
    let term = Terminator {
        kind: TerminatorKind::Return,
        span: Span::dummy(),
    };
    let opts = PrettyPrintOptions {
        include_spans: true,
        include_debug_names: false,
    };
    let p = PrettyPrinter::with_options(&m, opts);
    assert_eq!(p.render_statement(&stmt, &body), "_3 = copy _2;");
    assert_eq!(p.render_terminator(&term, &body), "return -> []");
}
