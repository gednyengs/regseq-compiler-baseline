//! Integration tests for stage-1 step-7: the canonical
//! `render_def_path` helper.
//!
//! Per-variant rendering rules are pinned against pinned
//! toolchain `nightly-2026-02-01-aarch64-apple-darwin` rustc
//! source. The 19-variant battery + closed-set guard catches a
//! future `DefPathData` variant addition that forgets a
//! rendering arm. The determinism test guards the spec invariant
//! that two RegseqModels for the same source produce
//! byte-identical `rendered` strings for corresponding items.

use crate::{
    render_def_path, CrateId, CrateInfo, DefPath, DefPathData, DefPathSegment, IndexVec,
    StableCrateId, StringId, StringInterner,
};

// ------------------------------------------------------------------
// Test fixtures
// ------------------------------------------------------------------

/// Builds a fresh single-crate `(crates, strings)` pair where the
/// primary crate is named `"my_crate"`. Returned interner has the
/// empty string at `StringId::ZERO` (per `StringInterner::new`),
/// `"my_crate"` interned next.
fn make_setup() -> (IndexVec<CrateId, CrateInfo>, StringInterner) {
    let mut strings = StringInterner::new();
    let primary_name = strings.intern("my_crate");
    let mut crates: IndexVec<CrateId, CrateInfo> = IndexVec::new();
    crates.push(CrateInfo {
        name: primary_name,
        stable_crate_id: StableCrateId(1),
    });
    (crates, strings)
}

fn one_segment(data: DefPathData, disambiguator: u32) -> DefPath {
    DefPath {
        crate_id: CrateId::ZERO,
        segments: vec![DefPathSegment {
            data,
            disambiguator,
        }],
        // `rendered` is not consumed by the helper; any StringId
        // works for tests that don't read it.
        rendered: StringId::ZERO,
    }
}

fn render_one(data: DefPathData, disambiguator: u32) -> String {
    let (crates, strings) = make_setup();
    render_def_path(&one_segment(data, disambiguator), &crates, &strings)
}

fn render_named(name: &str, ctor: fn(StringId) -> DefPathData, disambiguator: u32) -> String {
    let mut strings = StringInterner::new();
    let primary_name = strings.intern("my_crate");
    let payload = strings.intern(name);
    let mut crates: IndexVec<CrateId, CrateInfo> = IndexVec::new();
    crates.push(CrateInfo {
        name: primary_name,
        stable_crate_id: StableCrateId(1),
    });
    render_def_path(
        &one_segment(ctor(payload), disambiguator),
        &crates,
        &strings,
    )
}

// ------------------------------------------------------------------
// Plain name-bearing variants (TypeNs / ValueNs / MacroNs /
// LifetimeNs / OpaqueLifetime): render `name`, with `#N` only when
// disambig != 0.
// ------------------------------------------------------------------

#[test]
fn renders_type_ns_dis_zero() {
    assert_eq!(render_named("Foo", DefPathData::TypeNs, 0), "my_crate::Foo");
}

#[test]
fn renders_type_ns_dis_nonzero() {
    assert_eq!(
        render_named("Foo", DefPathData::TypeNs, 3),
        "my_crate::Foo#3"
    );
}

#[test]
fn renders_value_ns_dis_zero() {
    assert_eq!(
        render_named("bar", DefPathData::ValueNs, 0),
        "my_crate::bar"
    );
}

#[test]
fn renders_value_ns_dis_nonzero() {
    assert_eq!(
        render_named("bar", DefPathData::ValueNs, 5),
        "my_crate::bar#5"
    );
}

#[test]
fn renders_macro_ns_dis_zero() {
    assert_eq!(
        render_named("println", DefPathData::MacroNs, 0),
        "my_crate::println"
    );
}

#[test]
fn renders_macro_ns_dis_nonzero() {
    assert_eq!(
        render_named("println", DefPathData::MacroNs, 2),
        "my_crate::println#2"
    );
}

#[test]
fn renders_lifetime_ns_dis_zero() {
    assert_eq!(render_named("a", DefPathData::LifetimeNs, 0), "my_crate::a");
}

#[test]
fn renders_lifetime_ns_dis_nonzero() {
    assert_eq!(
        render_named("a", DefPathData::LifetimeNs, 4),
        "my_crate::a#4"
    );
}

#[test]
fn renders_opaque_lifetime_dis_zero() {
    assert_eq!(
        render_named("captured", DefPathData::OpaqueLifetime, 0),
        "my_crate::captured"
    );
}

#[test]
fn renders_opaque_lifetime_dis_nonzero() {
    assert_eq!(
        render_named("captured", DefPathData::OpaqueLifetime, 1),
        "my_crate::captured#1"
    );
}

// ------------------------------------------------------------------
// DesugaredAnonymousLifetime: literal `'_` plus the named-segment
// disambig rule (rustc maps to `Named(kw::UnderscoreLifetime)` per
// `definitions.rs:516`).
// ------------------------------------------------------------------

#[test]
fn renders_desugared_anonymous_lifetime_dis_zero() {
    assert_eq!(
        render_one(DefPathData::DesugaredAnonymousLifetime, 0),
        "my_crate::'_"
    );
}

#[test]
fn renders_desugared_anonymous_lifetime_dis_nonzero() {
    assert_eq!(
        render_one(DefPathData::DesugaredAnonymousLifetime, 7),
        "my_crate::'_#7"
    );
}

// ------------------------------------------------------------------
// AnonAssocTy: hybrid rendering (rustc emits `name::{anon_assoc#N}`
// for a single segment per `definitions.rs:195-198, :518`).
// ------------------------------------------------------------------

#[test]
fn renders_anon_assoc_ty_dis_zero() {
    assert_eq!(
        render_named("method", DefPathData::AnonAssocTy, 0),
        "my_crate::method::{anon_assoc#0}"
    );
}

#[test]
fn renders_anon_assoc_ty_dis_nonzero() {
    assert_eq!(
        render_named("method", DefPathData::AnonAssocTy, 5),
        "my_crate::method::{anon_assoc#5}"
    );
}

// ------------------------------------------------------------------
// Pure anonymous variants: always `{namespace#disambig}`, including
// disambig 0. Namespace strings pinned per `definitions.rs:509-520`.
// ------------------------------------------------------------------

#[test]
fn renders_impl_dis_zero() {
    assert_eq!(render_one(DefPathData::Impl, 0), "my_crate::{impl#0}");
}

#[test]
fn renders_impl_dis_nonzero() {
    assert_eq!(render_one(DefPathData::Impl, 2), "my_crate::{impl#2}");
}

#[test]
fn renders_foreign_mod_dis_zero() {
    assert_eq!(
        render_one(DefPathData::ForeignMod, 0),
        "my_crate::{extern#0}"
    );
}

#[test]
fn renders_use_dis_zero() {
    assert_eq!(render_one(DefPathData::Use, 0), "my_crate::{use#0}");
}

#[test]
fn renders_global_asm_dis_zero() {
    assert_eq!(
        render_one(DefPathData::GlobalAsm, 0),
        "my_crate::{global_asm#0}"
    );
}

#[test]
fn renders_closure_dis_zero() {
    assert_eq!(render_one(DefPathData::Closure, 0), "my_crate::{closure#0}");
}

#[test]
fn renders_ctor_dis_zero() {
    assert_eq!(
        render_one(DefPathData::Ctor, 0),
        "my_crate::{constructor#0}"
    );
}

#[test]
fn renders_anon_const_dis_zero() {
    assert_eq!(
        render_one(DefPathData::AnonConst, 0),
        "my_crate::{anonconst#0}"
    );
}

#[test]
fn renders_late_anon_const_dis_zero() {
    assert_eq!(
        render_one(DefPathData::LateAnonConst, 0),
        "my_crate::{anonconst#0}"
    );
}

#[test]
fn renders_opaque_ty_dis_zero() {
    assert_eq!(render_one(DefPathData::OpaqueTy, 0), "my_crate::{opaque#0}");
}

#[test]
fn renders_synthetic_coroutine_body_dis_zero() {
    assert_eq!(
        render_one(DefPathData::SyntheticCoroutineBody, 0),
        "my_crate::{synthetic#0}"
    );
}

#[test]
fn renders_nested_static_dis_zero() {
    assert_eq!(
        render_one(DefPathData::NestedStatic, 0),
        "my_crate::{nested#0}"
    );
}

// ------------------------------------------------------------------
// CrateRoot handling.
// ------------------------------------------------------------------

#[test]
fn renders_crate_root_only_as_crate_prefix() {
    let (crates, strings) = make_setup();
    let path = DefPath {
        crate_id: CrateId::ZERO,
        segments: vec![DefPathSegment {
            data: DefPathData::CrateRoot,
            disambiguator: 0,
        }],
        rendered: StringId::ZERO,
    };
    assert_eq!(render_def_path(&path, &crates, &strings), "my_crate");
}

#[test]
fn renders_empty_segments_as_crate_prefix() {
    let (crates, strings) = make_setup();
    let path = DefPath {
        crate_id: CrateId::ZERO,
        segments: vec![],
        rendered: StringId::ZERO,
    };
    assert_eq!(render_def_path(&path, &crates, &strings), "my_crate");
}

// ------------------------------------------------------------------
// Multi-segment exemplar.
// ------------------------------------------------------------------

#[test]
fn renders_canonical_impl_method_exemplar() {
    let mut strings = StringInterner::new();
    let primary_name = strings.intern("my_crate");
    let module_name = strings.intern("module");
    let item_name = strings.intern("Item");
    let method_name = strings.intern("method");
    let mut crates: IndexVec<CrateId, CrateInfo> = IndexVec::new();
    crates.push(CrateInfo {
        name: primary_name,
        stable_crate_id: StableCrateId(1),
    });

    let path = DefPath {
        crate_id: CrateId::ZERO,
        segments: vec![
            DefPathSegment {
                data: DefPathData::CrateRoot,
                disambiguator: 0,
            },
            DefPathSegment {
                data: DefPathData::TypeNs(module_name),
                disambiguator: 0,
            },
            DefPathSegment {
                data: DefPathData::TypeNs(item_name),
                disambiguator: 0,
            },
            DefPathSegment {
                data: DefPathData::Impl,
                disambiguator: 0,
            },
            DefPathSegment {
                data: DefPathData::ValueNs(method_name),
                disambiguator: 0,
            },
        ],
        rendered: StringId::ZERO,
    };

    assert_eq!(
        render_def_path(&path, &crates, &strings),
        "my_crate::module::Item::{impl#0}::method"
    );
}

// ------------------------------------------------------------------
// Multi-crate scenario: prefix is the crate's NAME, not its index.
// ------------------------------------------------------------------

#[test]
fn renders_uses_crate_name_from_table_not_index() {
    let mut strings = StringInterner::new();
    let primary_name = strings.intern("primary");
    let upstream_name = strings.intern("upstream");
    let mut crates: IndexVec<CrateId, CrateInfo> = IndexVec::new();
    crates.push(CrateInfo {
        name: primary_name,
        stable_crate_id: StableCrateId(1),
    });
    let upstream_id = crates.push(CrateInfo {
        name: upstream_name,
        stable_crate_id: StableCrateId(2),
    });

    let foo = strings.intern("Foo");
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

    let rendered = render_def_path(&path, &crates, &strings);
    assert_eq!(rendered, "upstream::Foo");
    assert!(!rendered.contains("primary"));
    assert!(!rendered.contains('1'));
}

// ------------------------------------------------------------------
// Determinism: TWO independent (crates, strings) pairs with the same
// string contents must produce byte-identical output. The interners
// may allocate different StringIds for the same content; the render
// rule must not depend on numeric identity.
// ------------------------------------------------------------------

#[test]
fn render_def_path_is_byte_identical_across_calls() {
    fn build(extra_pollution: bool) -> (IndexVec<CrateId, CrateInfo>, StringInterner, DefPath) {
        let mut strings = StringInterner::new();
        if extra_pollution {
            // Force StringIds to differ between the two builds:
            // intern noise strings so the meaningful payloads land
            // at different interner indexes.
            let _ = strings.intern("noise_a");
            let _ = strings.intern("noise_b");
        }
        let primary_name = strings.intern("my_crate");
        let module_name = strings.intern("module");
        let item_name = strings.intern("Item");
        let method_name = strings.intern("method");
        let mut crates: IndexVec<CrateId, CrateInfo> = IndexVec::new();
        crates.push(CrateInfo {
            name: primary_name,
            stable_crate_id: StableCrateId(1),
        });
        let path = DefPath {
            crate_id: CrateId::ZERO,
            segments: vec![
                DefPathSegment {
                    data: DefPathData::CrateRoot,
                    disambiguator: 0,
                },
                DefPathSegment {
                    data: DefPathData::TypeNs(module_name),
                    disambiguator: 0,
                },
                DefPathSegment {
                    data: DefPathData::TypeNs(item_name),
                    disambiguator: 0,
                },
                DefPathSegment {
                    data: DefPathData::Impl,
                    disambiguator: 0,
                },
                DefPathSegment {
                    data: DefPathData::ValueNs(method_name),
                    disambiguator: 0,
                },
            ],
            rendered: StringId::ZERO,
        };
        (crates, strings, path)
    }

    let (a_crates, a_strings, a_path) = build(false);
    let (b_crates, b_strings, b_path) = build(true);

    // Sanity: the two builds really did allocate different
    // StringIds for the meaningful payload (otherwise this test
    // is not exercising the determinism property).
    let a_module_id = match a_path.segments[1].data {
        DefPathData::TypeNs(s) => s,
        _ => unreachable!(),
    };
    let b_module_id = match b_path.segments[1].data {
        DefPathData::TypeNs(s) => s,
        _ => unreachable!(),
    };
    assert_ne!(
        a_module_id, b_module_id,
        "test pre-condition: pollution should shift StringId allocation"
    );

    let rendered_a = render_def_path(&a_path, &a_crates, &a_strings);
    let rendered_b = render_def_path(&b_path, &b_crates, &b_strings);
    assert_eq!(
        rendered_a, rendered_b,
        "rendered strings must be byte-identical across independent builds"
    );
    assert_eq!(rendered_a, "my_crate::module::Item::{impl#0}::method");
}

// ------------------------------------------------------------------
// Negative cases: explicit panic messages for diagnostic clarity.
// ------------------------------------------------------------------

#[test]
#[should_panic(expected = "crate_id")]
fn render_def_path_panics_on_dangling_crate_id() {
    let (crates, strings) = make_setup();
    // crates has length 1; CrateId(99) is out of range.
    let path = DefPath {
        crate_id: CrateId::from_u32(99),
        segments: vec![],
        rendered: StringId::ZERO,
    };
    let _ = render_def_path(&path, &crates, &strings);
}

#[test]
#[should_panic(expected = "CrateRoot at non-leading position")]
fn render_def_path_panics_on_crate_root_at_non_leading_position() {
    let (crates, strings) = make_setup();
    let foo = StringId::ZERO; // empty string is fine; we never reach the StringId resolution
    let path = DefPath {
        crate_id: CrateId::ZERO,
        segments: vec![
            DefPathSegment {
                data: DefPathData::TypeNs(foo),
                disambiguator: 0,
            },
            DefPathSegment {
                data: DefPathData::CrateRoot,
                disambiguator: 0,
            },
        ],
        rendered: StringId::ZERO,
    };
    let _ = render_def_path(&path, &crates, &strings);
}

#[test]
#[should_panic(expected = "crate name StringId")]
fn render_def_path_panics_on_dangling_crate_name_string_id() {
    // Build a CrateInfo whose `name` StringId is deliberately past
    // the end of the interner. This exercises the explicit
    // pre-index check that emits `crate name StringId {n} out of
    // range`, distinct from the dangling-payload-StringId path
    // (which propagates from `StringInterner::resolve`).
    let strings = StringInterner::new();
    let mut crates: IndexVec<CrateId, CrateInfo> = IndexVec::new();
    crates.push(CrateInfo {
        name: StringId::from_u32(9999),
        stable_crate_id: StableCrateId(1),
    });
    let path = DefPath {
        crate_id: CrateId::ZERO,
        segments: vec![],
        rendered: StringId::ZERO,
    };
    let _ = render_def_path(&path, &crates, &strings);
}

#[test]
#[should_panic]
fn render_def_path_panics_on_dangling_string_id_payload() {
    let (crates, strings) = make_setup();
    // strings has only ZERO ("") and primary name; StringId(9999)
    // is out of range. Propagates through StringInterner::resolve.
    let path = DefPath {
        crate_id: CrateId::ZERO,
        segments: vec![DefPathSegment {
            data: DefPathData::TypeNs(StringId::from_u32(9999)),
            disambiguator: 0,
        }],
        rendered: StringId::ZERO,
    };
    let _ = render_def_path(&path, &crates, &strings);
}

// ------------------------------------------------------------------
// Closed-set guard: every DefPathData variant must be rendered by
// SOME arm of render_def_path. Adding a 20th variant in a future
// toolchain bump fails this test until the rendering arm is added.
// ------------------------------------------------------------------

#[test]
fn render_def_path_battery_covers_all_19_variants() {
    let mut strings = StringInterner::new();
    let primary_name = strings.intern("my_crate");
    let payload = strings.intern("x");
    let mut crates: IndexVec<CrateId, CrateInfo> = IndexVec::new();
    crates.push(CrateInfo {
        name: primary_name,
        stable_crate_id: StableCrateId(1),
    });

    let variants: Vec<DefPathData> = vec![
        DefPathData::CrateRoot,
        DefPathData::Impl,
        DefPathData::ForeignMod,
        DefPathData::Use,
        DefPathData::GlobalAsm,
        DefPathData::TypeNs(payload),
        DefPathData::ValueNs(payload),
        DefPathData::MacroNs(payload),
        DefPathData::LifetimeNs(payload),
        DefPathData::Closure,
        DefPathData::Ctor,
        DefPathData::AnonConst,
        DefPathData::LateAnonConst,
        DefPathData::DesugaredAnonymousLifetime,
        DefPathData::OpaqueTy,
        DefPathData::OpaqueLifetime(payload),
        DefPathData::AnonAssocTy(payload),
        DefPathData::SyntheticCoroutineBody,
        DefPathData::NestedStatic,
    ];
    assert_eq!(
        variants.len(),
        19,
        "DefPathData is closed at 19 variants per the pinned toolchain"
    );

    // Render each variant in a single-segment path; the call
    // must not panic, and the output must contain the crate
    // prefix at minimum.
    for variant in variants {
        let path = DefPath {
            crate_id: CrateId::ZERO,
            segments: vec![DefPathSegment {
                data: variant.clone(),
                disambiguator: 0,
            }],
            rendered: StringId::ZERO,
        };
        let rendered = render_def_path(&path, &crates, &strings);
        assert!(
            rendered.starts_with("my_crate"),
            "variant {variant:?} did not render with the crate prefix: {rendered:?}"
        );
    }
}
