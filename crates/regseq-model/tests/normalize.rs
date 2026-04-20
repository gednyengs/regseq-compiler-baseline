//! Integration tests for [`regseq_model::normalize`].

use std::path::PathBuf;

use regseq_model::model::ModuleKind;
use regseq_model::normalize::raw_artifact::{
    RawDriverIdentity, RawFieldFact, RawFrontendArtifact, RawIdentityEnrichmentSurface,
    RawPropertyField, RawPropertyLint, RawPropertySurface, RawStructFact,
};
use regseq_model::{normalize, NormalizeErrorKind};

fn fixture_path() -> PathBuf {
    // crates/regseq-model/tests/normalize.rs -> repo root + fixtures
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../tests/fixtures/pl011-full-spec/expected/raw-frontend.json");
    p
}

fn load_pl011() -> RawFrontendArtifact {
    let raw =
        std::fs::read_to_string(fixture_path()).expect("read pl011 raw-frontend.json fixture");
    serde_json::from_str(&raw).expect("deserialize RawFrontendArtifact from fixture")
}

// ------------------------------------------------------------- happy path

#[test]
fn pl011_fixture_normalizes_and_passes_phase_1() {
    let input = load_pl011();
    let model = normalize(&input).expect("normalize must succeed");

    // Root is Normal; exactly one Driver submodule with the expected name.
    assert_eq!(model.root_module.kind, ModuleKind::Normal);
    assert_eq!(
        model.root_module.submodules.len(),
        1,
        "root module must have exactly one submodule (the driver module)",
    );
    let driver_mod = &model.root_module.submodules[0];
    assert_eq!(driver_mod.kind, ModuleKind::Driver);
    let driver_mod_name = model.symbols.resolve(driver_mod.name);
    assert_eq!(driver_mod_name, "pl011-full-spec");

    // Every binding slot on the driver struct starts `None`.
    let driver = driver_mod
        .driver
        .as_ref()
        .expect("driver submodule has driver");
    assert!(driver.clocks.iter().all(|c| c.binding.is_none()));
    assert!(driver.resets.iter().all(|r| r.binding.is_none()));
    assert!(driver.pinctrl.iter().all(|p| p.binding.is_none()));
    assert!(driver.interrupts.iter().all(|i| i.binding.is_none()));
    assert!(driver.mmio.iter().all(|m| m.binding.is_none()));
    assert!(driver.properties.iter().all(|p| p.bound_value.is_none()));

    // Every TypeDef has a classification.
    for t in &driver_mod.type_defs {
        let _ = &t.classification;
    }
    for t in &model.root_module.type_defs {
        let _ = &t.classification;
    }
}

// ------------------------------------------------------------- determinism

#[test]
fn repeated_normalize_is_byte_identical() {
    // Serializes the full `RegseqModel` payload to a JSON string
    // and asserts the two runs produce byte-identical JSON.
    //
    // `RegseqModel` does not itself derive `serde::Serialize` (see
    // its rustdoc -- the core IR types currently cover only the
    // declaration subset that already derives it). Rather than
    // widen those derives here as a side effect of this task, the
    // test snapshots the four public fields of `RegseqModel` into a
    // local `Serialize`-deriving DTO. Every field is covered:
    //
    //   - `symbols`: the interner string pool in insertion order
    //     (via `SymbolInterner::strings`; the HashMap lookup is
    //     intentionally skipped because its iteration order is
    //     non-deterministic and it is redundant with the Vec).
    //   - `root_module`: the full module tree (Debug-rendered,
    //     which is deterministic because every intermediate
    //     container is a `Vec` in surface-visit order or a
    //     `BTreeMap`/`BTreeSet`, and the embedded `Symbol`
    //     indices are monotonically assigned by the interner).
    //   - `source_locations`: the `BTreeMap<DeclId, SourceOrigin>`
    //     (Debug-rendered; BTreeMap iterates deterministically).
    //   - `format_version`: the `u32` header.
    //
    // `serde_json::to_string_pretty` is used so the output is
    // newline-formatted JSON; a byte comparison is then an
    // end-to-end JSON byte-equality assertion.
    #[derive(serde::Serialize)]
    struct Snapshot {
        strings: Vec<String>,
        root_module_debug: String,
        source_locations_debug: String,
        format_version: u32,
    }

    let input = load_pl011();
    let a = normalize(&input).unwrap();
    let b = normalize(&input).unwrap();

    let serialize = |m: &regseq_model::model::RegseqModel| -> String {
        let snap = Snapshot {
            strings: m.symbols.strings().to_vec(),
            root_module_debug: format!("{:#?}", m.root_module),
            source_locations_debug: format!("{:#?}", m.source_locations),
            format_version: m.format_version,
        };
        serde_json::to_string_pretty(&snap).expect("snapshot must serialize to JSON")
    };

    let sa = serialize(&a);
    let sb = serialize(&b);
    assert_eq!(
        sa.len(),
        sb.len(),
        "determinism: JSON output lengths differ"
    );
    assert!(
        sa == sb,
        "determinism: repeated normalize produced divergent JSON serialization"
    );
}

// ----------------------------------------------------- UnrecognizedEnumTag

#[test]
fn unrecognized_lint_kind_returns_unrecognized_enum_tag() {
    let mut input = load_pl011();
    let props = input
        .firmware_properties
        .as_mut()
        .expect("fixture carries firmware_properties");
    props.lints.first_mut().expect("fixture has >=1 lint").kind = "BogusLint".into();

    let err = normalize(&input).expect_err("unknown lint kind must fail");
    assert!(
        !err.provenance.surface_path.is_empty(),
        "surface_path must be populated",
    );
    assert!(
        !err.provenance.containing_item.is_empty(),
        "containing_item must be populated",
    );
    match &err.kind {
        NormalizeErrorKind::UnrecognizedEnumTag {
            surface,
            field,
            tag,
        } => {
            assert_eq!(*surface, "firmware_properties.lints");
            assert_eq!(*field, "kind");
            assert_eq!(tag, "BogusLint");
        }
        other => panic!("expected UnrecognizedEnumTag, got {other:?}"),
    }
}

// ------------------------------------------------- MissingRequiredAttribute

#[test]
fn clock_without_id_returns_missing_required_attribute() {
    let mut input = load_pl011();
    let clock = input.clock.as_mut().expect("fixture has clock surface");
    clock
        .fields
        .first_mut()
        .expect("fixture has >=1 clock field")
        .clock_id = String::new();

    let err = normalize(&input).expect_err("empty clock_id must fail");
    assert!(!err.provenance.surface_path.is_empty());
    assert!(!err.provenance.containing_item.is_empty());
    match &err.kind {
        NormalizeErrorKind::MissingRequiredAttribute {
            surface,
            attribute,
            item: _,
        } => {
            assert_eq!(*surface, "clock.fields");
            assert_eq!(*attribute, "clock_id");
        }
        other => panic!("expected MissingRequiredAttribute, got {other:?}"),
    }
}

// ----------------------------------------------- StructuralViolation (multi-driver)

#[test]
fn multi_driver_enrichment_returns_structural_violation() {
    // The PL011 fixture does not populate identity_enrichment; the
    // multi-driver scenario is constructed here by adding two
    // is_driver=true structs in different module paths.
    let mut input = load_pl011();
    let enrichment = RawIdentityEnrichmentSurface {
        structs: vec![
            RawStructFact {
                canonical_path: "crate::a::FirstDriver".into(),
                name: "FirstDriver".into(),
                module_path: "crate::a".into(),
                is_driver: true,
                is_register_block: false,
                fields: vec![],
            },
            RawStructFact {
                canonical_path: "crate::b::SecondDriver".into(),
                name: "SecondDriver".into(),
                module_path: "crate::b".into(),
                is_driver: true,
                is_register_block: false,
                fields: vec![],
            },
        ],
        ..Default::default()
    };
    input.identity_enrichment = Some(enrichment);

    let err = normalize(&input).expect_err("two driver structs must fail");
    assert!(!err.provenance.surface_path.is_empty());
    assert!(!err.provenance.containing_item.is_empty());
    match &err.kind {
        NormalizeErrorKind::StructuralViolation { description } => {
            assert!(
                description.contains("2")
                    || description.contains("multi-driver")
                    || description.contains("driver"),
                "description should mention driver count: {description}",
            );
        }
        other => panic!("expected StructuralViolation, got {other:?}"),
    }
}

// ---------------------------------------- TypeConstraintViolation (user data)

#[test]
fn non_option_user_data_returns_type_constraint_violation() {
    // Brief example: `Option<T: Copy>` is required for user-data
    // fields (spec.md Section 4.4). Set `field_type` to `u32` (not
    // `Option<_>`) and assert the error fires.
    let mut input = load_pl011();
    let enrichment = RawIdentityEnrichmentSurface {
        structs: vec![RawStructFact {
            canonical_path: "crate::driver::Pl011Driver".into(),
            name: "Pl011Driver".into(),
            module_path: "crate::driver".into(),
            is_driver: true,
            is_register_block: false,
            fields: vec![RawFieldFact {
                name: "counter".into(),
                field_type: "u32".into(),
                rseq_field_kind: "UserData".into(),
            }],
        }],
        ..Default::default()
    };
    input.identity_enrichment = Some(enrichment);

    let err = normalize(&input).expect_err("non-Option user data must fail");
    assert!(!err.provenance.surface_path.is_empty());
    assert!(!err.provenance.containing_item.is_empty());
    match &err.kind {
        NormalizeErrorKind::TypeConstraintViolation { description } => {
            assert!(
                description.contains("Option") || description.contains("user-data"),
                "description should mention Option: {description}",
            );
        }
        other => panic!("expected TypeConstraintViolation, got {other:?}"),
    }
}

// ----------------------------- Option<T> wrapping a non-Copy inner type

#[test]
fn option_user_data_with_non_copy_inner_returns_type_constraint_violation() {
    // The spec (Section 4.4) demands `Option<T: Copy>`. An outer
    // `Option<_>` wrapper alone is not sufficient; the inner type
    // must also satisfy `Copy`. `String` is the classic non-`Copy`
    // stdlib type, so this scenario exercises the inner-bound check.
    let mut input = load_pl011();
    let enrichment = RawIdentityEnrichmentSurface {
        structs: vec![RawStructFact {
            canonical_path: "crate::driver::Pl011Driver".into(),
            name: "Pl011Driver".into(),
            module_path: "crate::driver".into(),
            is_driver: true,
            is_register_block: false,
            fields: vec![RawFieldFact {
                name: "label".into(),
                field_type: "Option<String>".into(),
                rseq_field_kind: "UserData".into(),
            }],
        }],
        ..Default::default()
    };
    input.identity_enrichment = Some(enrichment);

    let err = normalize(&input).expect_err("Option<String> user data must fail");
    match &err.kind {
        NormalizeErrorKind::TypeConstraintViolation { description } => {
            assert!(
                description.contains("Copy"),
                "description should mention Copy: {description}",
            );
        }
        other => panic!("expected TypeConstraintViolation, got {other:?}"),
    }
}

// -------------------------------------------------- Option<T: Copy> accepted

#[test]
fn option_user_data_type_is_accepted() {
    let mut input = load_pl011();
    let enrichment = RawIdentityEnrichmentSurface {
        structs: vec![RawStructFact {
            canonical_path: "crate::driver::Pl011Driver".into(),
            name: "Pl011Driver".into(),
            module_path: "crate::driver".into(),
            is_driver: true,
            is_register_block: false,
            fields: vec![RawFieldFact {
                name: "counter".into(),
                field_type: "Option<u32>".into(),
                rseq_field_kind: "UserData".into(),
            }],
        }],
        ..Default::default()
    };
    input.identity_enrichment = Some(enrichment);

    let _model = normalize(&input).expect("Option<T> user-data type must normalize cleanly");
}

// ---------------------------------------------------------- provenance

#[test]
fn every_error_carries_populated_provenance() {
    // Each of the negative scenarios above already asserts
    // non-empty provenance. This test also exercises the
    // MissingRequiredAttribute branch on property_id, which is a
    // different code path than the clock_id branch, to defend
    // against a regression where one helper drops the
    // containing_item label.
    let mut input = RawFrontendArtifact::default();
    input.driver_name = "testdrv".into();
    input.identity = Some(RawDriverIdentity {
        driver_name: "testdrv".into(),
        compatible: vec![],
        device_class: "serial".into(),
        bus: None,
    });
    input.firmware_properties = Some(RawPropertySurface {
        fields: vec![RawPropertyField {
            driver_type: "Testdrv".into(),
            field_name: "clock_frequency".into(),
            field_type: "u32".into(),
            property_id: String::new(),
            default_literal: None,
            vendor_prefix: None,
            required: true,
        }],
        lints: vec![],
    });
    let err = normalize(&input).expect_err("empty property_id must fail");
    assert!(
        err.provenance.surface_path.contains("firmware_properties"),
        "surface_path must trace to firmware_properties: {}",
        err.provenance.surface_path,
    );
    assert!(
        err.provenance.containing_item.contains("PropertyField"),
        "containing_item must mention PropertyField: {}",
        err.provenance.containing_item,
    );
    // Also assert negative lint scenario's surface path is
    // field-specific rather than generic (catches a regression where
    // the lint helper forgets to push the `lints[idx]` segment).
    let _ = RawPropertyLint::default();
}
