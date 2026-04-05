#![feature(rustc_private)]

extern crate rustc_driver;

use regseq_driver::mir_repr::*;
use regseq_driver::raw_artifact::*;

fn sample_artifact() -> RawFrontendArtifact {
    RawFrontendArtifact {
        driver_name: "pl011-spec-slice".to_string(),
        target_os: "linux".to_string(),
        identity: Some(RawDriverIdentity {
            driver_name: "pl011-spec-slice".to_string(),
            compatible: vec!["arm,pl011".to_string()],
            device_class: "serial".to_string(),
            bus: Some("amba".to_string()),
        }),
        register_access: Some(RawRegisterAccessSurface {
            register_blocks: vec![RawRegisterBlockDecl {
                name: "Pl011Registers".to_string(),
                fields: vec![
                    RawRegisterFieldDecl {
                        name: "dr".to_string(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: Some("read".to_string()),
                        hw_updates: None,
                    },
                    RawRegisterFieldDecl {
                        name: "fr".to_string(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: Some("volatile".to_string()),
                    },
                ],
            }],
            mmio_fields: vec![RawMmioFieldDecl {
                field_name: "regs".to_string(),
                mmio_id: Some("uart_regs".to_string()),
                register_block_type: Some("Pl011Registers".to_string()),
            }],
            wrapper_call_sites: vec![
                RawWrapperCallSite {
                    method: "read".to_string(),
                    containing_fn: "tx_ready".to_string(),
                },
                RawWrapperCallSite {
                    method: "write".to_string(),
                    containing_fn: "tx_byte".to_string(),
                },
            ],
            poll_call_sites: vec![RawPollCallSite {
                containing_fn: "wait_tx_complete".to_string(),
            }],
            register_descriptors: vec![RawRegisterDescriptorFact {
                register_module: "DR_REG".to_string(),
                fields: vec![RawRegisterDescriptorFieldFact {
                    field_name: "DATA".to_string(),
                    offset: 0,
                    width: 8,
                    named_values: vec![],
                }],
            }],
        }),
        trait_callbacks: Some(RawTraitSurface {
            lifecycle_impls: vec![RawTraitImpl {
                driver_type: "Pl011Driver".to_string(),
                context: "F".to_string(),
            }],
            serial_impls: vec![RawTraitImpl {
                driver_type: "Pl011Driver".to_string(),
                context: "S".to_string(),
            }],
            serial_console_impls: vec![],
            power_management_impls: vec![],
        }),
        firmware_properties: Some(RawPropertySurface {
            fields: vec![RawPropertyField {
                driver_type: "Pl011Driver".to_string(),
                field_name: "baud_rate".to_string(),
                field_type: "u32".to_string(),
                property_id: "baud-rate".to_string(),
                default_literal: Some("115200".to_string()),
                vendor_prefix: None,
                required: true,
            }],
            lints: vec![RawPropertyLint::RequiredWithDefault {
                driver_type: "Pl011Driver".to_string(),
                field_name: "baud_rate".to_string(),
                property_id: "baud-rate".to_string(),
            }],
        }),
        clock: Some(RawClockSurface {
            fields: vec![RawClockField {
                field_name: "uartclk".to_string(),
                clock_id: "uartclk".to_string(),
                required: true,
            }],
            call_sites: vec![RawClockCallSite {
                method: "enable".to_string(),
                containing_fn: "init".to_string(),
            }],
        }),
        reset: Some(RawResetSurface {
            fields: vec![],
            call_sites: vec![],
        }),
        pinctrl: Some(RawPinCtrlSurface {
            fields: vec![RawPinCtrlField {
                field_name: "pins".to_string(),
                required: false,
                pinctrl_id: Some("default".to_string()),
            }],
            call_sites: vec![],
        }),
        interrupts: None,
        synchronization: None,
        timed_delay: None,
        logging: None,
        pm_gate: None,
        bodies: None,
    }
}

/// Minimal MIR body for testing: a single basic block with Return.
fn minimal_mir_body() -> MirBody {
    MirBody {
        local_decls: vec![MirLocalDecl {
            index: 0,
            ty: MirTy::Tuple { elements: vec![] },
            mutability: MirMutability::Mut,
            name: Some("_0".to_string()),
        }],
        arg_count: 0,
        basic_blocks: vec![MirBasicBlockData {
            statements: vec![],
            terminator: Some(MirTerminator {
                source_info: MirSourceInfo {
                    scope: 0,
                    span: None,
                },
                kind: MirTerminatorKind::Return,
            }),
            is_cleanup: false,
        }],
        source_scopes: vec![MirSourceScope {
            index: 0,
            name: None,
        }],
    }
}

fn sample_body_surface() -> RawBodySurface {
    MirBodySurface {
        functions: vec![
            MirExtractedFn {
                name: "init".to_string(),
                def_path: "Pl011Driver::init".to_string(),
                kind: MirFnKind::Callback {
                    trait_name: "Lifecycle".to_string(),
                    method_name: "init".to_string(),
                },
                receiver: "ref_mut".to_string(),
                params: vec![],
                return_type: Some("Result<(), RegseqError>".to_string()),
                body: minimal_mir_body(),
                helper_calls: vec!["disable_uart".to_string()],
            },
            MirExtractedFn {
                name: "disable_uart".to_string(),
                def_path: "Pl011Driver::disable_uart".to_string(),
                kind: MirFnKind::Helper,
                receiver: "ref_mut".to_string(),
                params: vec![],
                return_type: None,
                body: minimal_mir_body(),
                helper_calls: vec![],
            },
            MirExtractedFn {
                name: "handle_irq".to_string(),
                def_path: "Pl011Driver::handle_irq".to_string(),
                kind: MirFnKind::Isr,
                receiver: "ref_mut".to_string(),
                params: vec![],
                return_type: Some("IrqResult".to_string()),
                body: minimal_mir_body(),
                helper_calls: vec![],
            },
        ],
    }
}

#[test]
fn raw_artifact_serializes_to_json() {
    let artifact = sample_artifact();
    let json = serde_json::to_string_pretty(&artifact).expect("should serialize");
    assert!(json.contains("\"driver_name\": \"pl011-spec-slice\""));
    assert!(json.contains("\"target_os\": \"linux\""));
    assert!(json.contains("\"Pl011Registers\""));
    assert!(json.contains("\"uart_regs\""));
}

#[test]
fn raw_artifact_round_trips_through_json() {
    let artifact = sample_artifact();
    let json = serde_json::to_string(&artifact).expect("serialize");
    let decoded: RawFrontendArtifact = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(artifact, decoded);
}

#[test]
fn raw_artifact_preserves_driver_identity() {
    let artifact = sample_artifact();
    let id = artifact.identity.as_ref().unwrap();
    assert_eq!(id.driver_name, "pl011-spec-slice");
    assert_eq!(id.compatible, vec!["arm,pl011"]);
    assert_eq!(id.device_class, "serial");
    assert_eq!(id.bus.as_deref(), Some("amba"));
}

#[test]
fn raw_artifact_bus_field_in_json() {
    let artifact = sample_artifact();
    let json = serde_json::to_string_pretty(&artifact).expect("serialize");
    assert!(json.contains("\"bus\": \"amba\""));
}

#[test]
fn raw_artifact_identity_without_bus_omits_field() {
    let artifact = RawFrontendArtifact {
        driver_name: "no-bus-driver".to_string(),
        target_os: "linux".to_string(),
        identity: Some(RawDriverIdentity {
            driver_name: "no-bus-driver".to_string(),
            compatible: vec!["vendor,dev".to_string()],
            device_class: "serial".to_string(),
            bus: None,
        }),
        register_access: None,
        trait_callbacks: None,
        firmware_properties: None,
        clock: None,
        reset: None,
        pinctrl: None,
        interrupts: None,
        synchronization: None,
        timed_delay: None,
        logging: None,
        pm_gate: None,
        bodies: None,
    };

    let json = serde_json::to_string_pretty(&artifact).expect("serialize");
    assert!(!json.contains("\"bus\""));

    let decoded: RawFrontendArtifact = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.identity.as_ref().unwrap().bus, None);
}

#[test]
fn raw_artifact_preserves_register_blocks() {
    let artifact = sample_artifact();
    let reg = artifact.register_access.as_ref().unwrap();
    assert_eq!(reg.register_blocks.len(), 1);
    assert_eq!(reg.register_blocks[0].name, "Pl011Registers");
    assert_eq!(reg.register_blocks[0].fields.len(), 2);
}

#[test]
fn raw_artifact_preserves_mmio_fields() {
    let artifact = sample_artifact();
    let reg = artifact.register_access.as_ref().unwrap();
    assert_eq!(reg.mmio_fields.len(), 1);
    assert_eq!(reg.mmio_fields[0].mmio_id.as_deref(), Some("uart_regs"));
    assert_eq!(
        reg.mmio_fields[0].register_block_type.as_deref(),
        Some("Pl011Registers")
    );
}

#[test]
fn raw_artifact_preserves_wrapper_call_sites() {
    let artifact = sample_artifact();
    let reg = artifact.register_access.as_ref().unwrap();
    assert_eq!(reg.wrapper_call_sites.len(), 2);
    assert_eq!(reg.wrapper_call_sites[0].method, "read");
    assert_eq!(reg.wrapper_call_sites[0].containing_fn, "tx_ready");
}

#[test]
fn raw_artifact_preserves_trait_callbacks() {
    let artifact = sample_artifact();
    let traits = artifact.trait_callbacks.as_ref().unwrap();
    assert_eq!(traits.lifecycle_impls.len(), 1);
    assert_eq!(traits.lifecycle_impls[0].context, "F");
    assert_eq!(traits.serial_impls.len(), 1);
    assert_eq!(traits.serial_impls[0].context, "S");
}

#[test]
fn raw_artifact_preserves_firmware_properties() {
    let artifact = sample_artifact();
    let props = artifact.firmware_properties.as_ref().unwrap();
    assert_eq!(props.fields.len(), 1);
    assert_eq!(props.fields[0].property_id, "baud-rate");
    assert_eq!(props.fields[0].default_literal.as_deref(), Some("115200"));
    assert!(props.fields[0].required);
}

#[test]
fn raw_artifact_preserves_clock_resource() {
    let artifact = sample_artifact();
    let clock = artifact.clock.as_ref().unwrap();
    assert_eq!(clock.fields.len(), 1);
    assert_eq!(clock.fields[0].clock_id, "uartclk");
    assert!(clock.fields[0].required);
}

#[test]
fn raw_artifact_preserves_pinctrl_resource() {
    let artifact = sample_artifact();
    let pinctrl = artifact.pinctrl.as_ref().unwrap();
    assert_eq!(pinctrl.fields.len(), 1);
    assert!(!pinctrl.fields[0].required);
    assert_eq!(pinctrl.fields[0].pinctrl_id.as_deref(), Some("default"));
}

#[test]
fn raw_artifact_absent_families_are_none() {
    let artifact = sample_artifact();
    assert!(artifact.interrupts.is_none());
    assert!(artifact.synchronization.is_none());
    assert!(artifact.timed_delay.is_none());
    assert!(artifact.logging.is_none());
    assert!(artifact.pm_gate.is_none());
}

#[test]
fn raw_artifact_empty_family_serializes_cleanly() {
    let artifact = RawFrontendArtifact {
        driver_name: "empty-driver".to_string(),
        target_os: "linux".to_string(),
        identity: None,
        register_access: None,
        trait_callbacks: None,
        firmware_properties: None,
        clock: None,
        reset: None,
        pinctrl: None,
        interrupts: None,
        synchronization: None,
        timed_delay: None,
        logging: None,
        pm_gate: None,
        bodies: None,
    };

    let json = serde_json::to_string(&artifact).expect("serialize");
    let decoded: RawFrontendArtifact = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.driver_name, "empty-driver");
    assert!(decoded.identity.is_none());
    assert!(decoded.register_access.is_none());
}

#[test]
fn raw_artifact_json_is_deterministic() {
    let artifact = sample_artifact();
    let json1 = serde_json::to_string_pretty(&artifact).expect("serialize 1");
    let json2 = serde_json::to_string_pretty(&artifact).expect("serialize 2");
    assert_eq!(json1, json2);
}

#[test]
fn emit_raw_artifact_to_temp_dir() {
    let artifact = sample_artifact();
    let tmp = std::env::temp_dir().join("regseq-raw-artifact-test");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let drivers_dir = tmp.join("drivers");
    std::fs::create_dir_all(&drivers_dir).unwrap();

    let path = drivers_dir.join(format!("{}.raw.json", artifact.driver_name));
    let json = serde_json::to_string_pretty(&artifact).expect("serialize");
    std::fs::write(&path, &json).expect("write");

    let read_back = std::fs::read_to_string(&path).expect("read back");
    let decoded: RawFrontendArtifact = serde_json::from_str(&read_back).expect("deserialize");
    assert_eq!(decoded, artifact);

    let _ = std::fs::remove_dir_all(&tmp);
}

// ---------------------------------------------------------------------------
// MIR body surface tests
// ---------------------------------------------------------------------------

#[test]
fn raw_artifact_mir_body_surface_round_trips() {
    let body = sample_body_surface();
    let json = serde_json::to_string_pretty(&body).expect("serialize");
    let decoded: RawBodySurface = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(body, decoded);
}

#[test]
fn raw_artifact_mir_body_captures_callback() {
    let body = sample_body_surface();
    let init_fn = body.functions.iter().find(|f| f.name == "init").unwrap();
    assert!(matches!(
        &init_fn.kind,
        MirFnKind::Callback {
            trait_name,
            method_name,
        } if trait_name == "Lifecycle" && method_name == "init"
    ));
    assert_eq!(init_fn.receiver, "ref_mut");
}

#[test]
fn raw_artifact_mir_body_captures_helper() {
    let body = sample_body_surface();
    let helper = body
        .functions
        .iter()
        .find(|f| f.name == "disable_uart")
        .unwrap();
    assert!(matches!(&helper.kind, MirFnKind::Helper));
    assert_eq!(helper.receiver, "ref_mut");
}

#[test]
fn raw_artifact_mir_body_captures_isr() {
    let body = sample_body_surface();
    let isr = body
        .functions
        .iter()
        .find(|f| f.name == "handle_irq")
        .unwrap();
    assert!(matches!(&isr.kind, MirFnKind::Isr));
    assert!(isr.return_type.as_deref() == Some("IrqResult"));
}

#[test]
fn raw_artifact_mir_body_preserves_provenance() {
    let body = sample_body_surface();
    let init_fn = body.functions.iter().find(|f| f.name == "init").unwrap();
    assert!(!init_fn.def_path.is_empty());
    assert_eq!(init_fn.def_path, "Pl011Driver::init");
}

#[test]
fn raw_artifact_mir_body_preserves_helper_calls() {
    let body = sample_body_surface();
    let init_fn = body.functions.iter().find(|f| f.name == "init").unwrap();
    assert_eq!(init_fn.helper_calls, vec!["disable_uart".to_string()]);
}

#[test]
fn raw_artifact_mir_body_preserves_basic_blocks() {
    let body = sample_body_surface();
    let init_fn = body.functions.iter().find(|f| f.name == "init").unwrap();
    assert!(!init_fn.body.basic_blocks.is_empty());
    let bb0 = &init_fn.body.basic_blocks[0];
    assert!(!bb0.is_cleanup);
    assert!(bb0.terminator.is_some());
    assert!(matches!(
        bb0.terminator.as_ref().unwrap().kind,
        MirTerminatorKind::Return
    ));
}

#[test]
fn raw_artifact_mir_body_preserves_local_decls() {
    let body = sample_body_surface();
    let init_fn = body.functions.iter().find(|f| f.name == "init").unwrap();
    assert!(!init_fn.body.local_decls.is_empty());
    // First local is return place
    assert_eq!(init_fn.body.local_decls[0].index, 0);
}

#[test]
fn raw_artifact_absent_bodies_is_none() {
    let artifact = sample_artifact();
    assert!(artifact.bodies.is_none());
}

#[test]
fn raw_artifact_with_mir_bodies_round_trips() {
    let mut artifact = sample_artifact();
    artifact.bodies = Some(sample_body_surface());
    let json = serde_json::to_string(&artifact).expect("serialize");
    let decoded: RawFrontendArtifact = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(artifact, decoded);
    assert!(decoded.bodies.is_some());
    assert_eq!(decoded.bodies.as_ref().unwrap().functions.len(), 3);
}

#[test]
fn raw_artifact_mir_unsupported_statement_preserved() {
    let stmt = MirStatementKind::Unsupported {
        description: "FakeIntrinsic".to_string(),
    };
    let wrapped = MirStatement {
        source_info: MirSourceInfo {
            scope: 0,
            span: None,
        },
        kind: stmt,
    };
    let json = serde_json::to_string_pretty(&wrapped).expect("serialize");
    let decoded: MirStatement = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(wrapped, decoded);
    assert!(json.contains("\"description\": \"FakeIntrinsic\""));
}
