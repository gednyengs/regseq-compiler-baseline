#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/register_access.rs"]
mod register_access;

use register_access::*;

type ExtractionResult = Result<RegisterAccessSurface, RegisterAccessError>;

static INIT: Once = Once::new();
static COMPILER_LOCK: Mutex<()> = Mutex::new(());

fn init_rustc() {
    INIT.call_once(|| {
        let early_dcx =
            rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
        rustc_driver::init_rustc_env_logger(&early_dcx);
    });
}

fn get_sysroot() -> String {
    let output = std::process::Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn compile_and_extract(source: &str, test_name: &str) -> ExtractionResult {
    let _lock = COMPILER_LOCK.lock().unwrap();
    init_rustc();

    let test_dir = std::env::temp_dir()
        .join("regseq-driver-tests")
        .join(test_name);
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
    std::fs::create_dir_all(&test_dir).unwrap();

    let src_path = test_dir.join("test_input.rs");
    std::fs::write(&src_path, source).unwrap();

    let sysroot = get_sysroot();

    let result: Arc<Mutex<Option<ExtractionResult>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        result: Arc<Mutex<Option<ExtractionResult>>>,
    }

    impl rustc_driver::Callbacks for ExtractCallbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("feature(register_tool)".to_string());
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("register_tool(rseq)".to_string());
            config.opts.unstable_opts.crate_attr.push(
                "allow(unused, non_snake_case, nonstandard_style, redundant_semicolons)"
                    .to_string(),
            );
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            *self.result.lock().unwrap() = Some(extract_register_access_surface(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
        result: result.clone(),
    };

    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "lib".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let extracted = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called");

    std::fs::remove_dir_all(&test_dir).ok();

    extracted
}

// ---------------------------------------------------------------------------
// Common preamble -- stub types matching the regseq facade
// ---------------------------------------------------------------------------

const FIXTURE_PREAMBLE: &str = r#"
    use core::marker::PhantomData;

    pub struct Field<T, R> { pub mask: T, pub shift: T, _r: PhantomData<R> }
    impl<T, R> Field<T, R> {
        pub const fn new(mask: T, shift: T) -> Self {
            Self { mask, shift, _r: PhantomData }
        }
    }

    pub struct FieldValue<T, R> { pub mask: T, pub value: T, _r: PhantomData<R> }
    impl<T, R> FieldValue<T, R> {
        pub const fn new(mask: T, value: T) -> Self {
            Self { mask, value, _r: PhantomData }
        }
    }
"#;

// ---------------------------------------------------------------------------
// Authored-fact table: every register, field, and named value from the
// PL011 fixture source (tests/fixtures/pl011-full-spec/src/registers.rs).
//
// This table is the single source of truth for the inline specimen and for
// every assertion. Any mismatch between this table and the fixture source
// is a blocking fidelity error.
// ---------------------------------------------------------------------------

/// Per-field authored fact.
struct AuthoredField {
    name: &'static str,
    offset: u32,
    width: u32,
    named_values: &'static [(&'static str, u32)],
}

/// Per-register authored fact.
struct AuthoredRegister {
    name: &'static str,
    fields: &'static [AuthoredField],
}

/// Complete authored register inventory from the PL011 fixture source.
static AUTHORED_REGISTERS: &[AuthoredRegister] = &[
    AuthoredRegister {
        name: "DR_REG",
        fields: &[
            AuthoredField {
                name: "OE",
                offset: 11,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "BE",
                offset: 10,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "PE",
                offset: 9,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "FE",
                offset: 8,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "DATA",
                offset: 0,
                width: 8,
                named_values: &[],
            },
        ],
    },
    AuthoredRegister {
        name: "RSR_ECR_REG",
        fields: &[
            AuthoredField {
                name: "OE",
                offset: 3,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "BE",
                offset: 2,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "PE",
                offset: 1,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "FE",
                offset: 0,
                width: 1,
                named_values: &[],
            },
        ],
    },
    AuthoredRegister {
        name: "FR_REG",
        fields: &[
            AuthoredField {
                name: "RI",
                offset: 8,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "TXFE",
                offset: 7,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RXFF",
                offset: 6,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "TXFF",
                offset: 5,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RXFE",
                offset: 4,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "BUSY",
                offset: 3,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "DCD",
                offset: 2,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "DSR",
                offset: 1,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "CTS",
                offset: 0,
                width: 1,
                named_values: &[],
            },
        ],
    },
    AuthoredRegister {
        name: "ILPR_REG",
        fields: &[AuthoredField {
            name: "ILPDVSR",
            offset: 0,
            width: 8,
            named_values: &[],
        }],
    },
    AuthoredRegister {
        name: "IBRD_REG",
        fields: &[AuthoredField {
            name: "BAUD_DIVINT",
            offset: 0,
            width: 16,
            named_values: &[],
        }],
    },
    AuthoredRegister {
        name: "FBRD_REG",
        fields: &[AuthoredField {
            name: "BAUD_DIVFRAC",
            offset: 0,
            width: 6,
            named_values: &[],
        }],
    },
    AuthoredRegister {
        name: "LCRH_REG",
        fields: &[
            AuthoredField {
                name: "SPS",
                offset: 7,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
            AuthoredField {
                name: "WLEN",
                offset: 5,
                width: 2,
                named_values: &[
                    ("FIVE_BITS", 0b00),
                    ("SIX_BITS", 0b01),
                    ("SEVEN_BITS", 0b10),
                    ("EIGHT_BITS", 0b11),
                ],
            },
            AuthoredField {
                name: "FEN",
                offset: 4,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
            AuthoredField {
                name: "STP2",
                offset: 3,
                width: 1,
                named_values: &[("ONE_STOP_BIT", 0), ("TWO_STOP_BITS", 1)],
            },
            AuthoredField {
                name: "EPS",
                offset: 2,
                width: 1,
                named_values: &[("ODD", 0), ("EVEN", 1)],
            },
            AuthoredField {
                name: "PEN",
                offset: 1,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
            AuthoredField {
                name: "BRK",
                offset: 0,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
        ],
    },
    AuthoredRegister {
        name: "CR_REG",
        fields: &[
            AuthoredField {
                name: "CTSEN",
                offset: 15,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
            AuthoredField {
                name: "RTSEN",
                offset: 14,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
            AuthoredField {
                name: "OUT2",
                offset: 13,
                width: 1,
                named_values: &[("INACTIVE", 0), ("ACTIVE", 1)],
            },
            AuthoredField {
                name: "OUT1",
                offset: 12,
                width: 1,
                named_values: &[("INACTIVE", 0), ("ACTIVE", 1)],
            },
            AuthoredField {
                name: "RTS",
                offset: 11,
                width: 1,
                named_values: &[("INACTIVE", 0), ("ACTIVE", 1)],
            },
            AuthoredField {
                name: "DTR",
                offset: 10,
                width: 1,
                named_values: &[("INACTIVE", 0), ("ACTIVE", 1)],
            },
            AuthoredField {
                name: "RXE",
                offset: 9,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
            AuthoredField {
                name: "TXE",
                offset: 8,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
            AuthoredField {
                name: "LBE",
                offset: 7,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
            AuthoredField {
                name: "SIRLP",
                offset: 2,
                width: 1,
                named_values: &[("NORMAL", 0), ("LOW_POWER", 1)],
            },
            AuthoredField {
                name: "SIREN",
                offset: 1,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
            AuthoredField {
                name: "UARTEN",
                offset: 0,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
        ],
    },
    AuthoredRegister {
        name: "IFLS_REG",
        fields: &[
            AuthoredField {
                name: "RXIFLSEL",
                offset: 3,
                width: 3,
                named_values: &[
                    ("ONE_EIGHTH", 0b000),
                    ("ONE_QUARTER", 0b001),
                    ("ONE_HALF", 0b010),
                    ("THREE_QUARTERS", 0b011),
                    ("SEVEN_EIGHTHS", 0b100),
                ],
            },
            AuthoredField {
                name: "TXIFLSEL",
                offset: 0,
                width: 3,
                named_values: &[
                    ("ONE_EIGHTH", 0b000),
                    ("ONE_QUARTER", 0b001),
                    ("ONE_HALF", 0b010),
                    ("THREE_QUARTERS", 0b011),
                    ("SEVEN_EIGHTHS", 0b100),
                ],
            },
        ],
    },
    AuthoredRegister {
        name: "IMSC_REG",
        fields: &[
            AuthoredField {
                name: "OEIM",
                offset: 10,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "BEIM",
                offset: 9,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "PEIM",
                offset: 8,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "FEIM",
                offset: 7,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RTIM",
                offset: 6,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "TXIM",
                offset: 5,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RXIM",
                offset: 4,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "DSRMIM",
                offset: 3,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "DCDMIM",
                offset: 2,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "CTSMIM",
                offset: 1,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RIMIM",
                offset: 0,
                width: 1,
                named_values: &[],
            },
        ],
    },
    AuthoredRegister {
        name: "RIS_REG",
        fields: &[
            AuthoredField {
                name: "OERIS",
                offset: 10,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "BERIS",
                offset: 9,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "PERIS",
                offset: 8,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "FERIS",
                offset: 7,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RTRIS",
                offset: 6,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "TXRIS",
                offset: 5,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RXRIS",
                offset: 4,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "DSRRMIS",
                offset: 3,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "DCDRMIS",
                offset: 2,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "CTSRMIS",
                offset: 1,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RIRMIS",
                offset: 0,
                width: 1,
                named_values: &[],
            },
        ],
    },
    AuthoredRegister {
        name: "MIS_REG",
        fields: &[
            AuthoredField {
                name: "OEMIS",
                offset: 10,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "BEMIS",
                offset: 9,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "PEMIS",
                offset: 8,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "FEMIS",
                offset: 7,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RTMIS",
                offset: 6,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "TXMIS",
                offset: 5,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RXMIS",
                offset: 4,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "DSRMMIS",
                offset: 3,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "DCDMMIS",
                offset: 2,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "CTSMMIS",
                offset: 1,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RIMMIS",
                offset: 0,
                width: 1,
                named_values: &[],
            },
        ],
    },
    AuthoredRegister {
        name: "ICR_REG",
        fields: &[
            AuthoredField {
                name: "OEIC",
                offset: 10,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "BEIC",
                offset: 9,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "PEIC",
                offset: 8,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "FEIC",
                offset: 7,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RTIC",
                offset: 6,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "TXIC",
                offset: 5,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RXIC",
                offset: 4,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "DSRMIC",
                offset: 3,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "DCDMIC",
                offset: 2,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "CTSMIC",
                offset: 1,
                width: 1,
                named_values: &[],
            },
            AuthoredField {
                name: "RIMIC",
                offset: 0,
                width: 1,
                named_values: &[],
            },
        ],
    },
    AuthoredRegister {
        name: "DMACR_REG",
        fields: &[
            AuthoredField {
                name: "DMAONERR",
                offset: 2,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
            AuthoredField {
                name: "TXDMAE",
                offset: 1,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
            AuthoredField {
                name: "RXDMAE",
                offset: 0,
                width: 1,
                named_values: &[("DISABLED", 0), ("ENABLED", 1)],
            },
        ],
    },
    AuthoredRegister {
        name: "PERIPHID0_REG",
        fields: &[AuthoredField {
            name: "PARTNUMBER0",
            offset: 0,
            width: 8,
            named_values: &[("PL011", 0x11)],
        }],
    },
    AuthoredRegister {
        name: "PERIPHID1_REG",
        fields: &[
            AuthoredField {
                name: "DESIGNER0",
                offset: 4,
                width: 4,
                named_values: &[("ARM_LOW", 0x1)],
            },
            AuthoredField {
                name: "PARTNUMBER1",
                offset: 0,
                width: 4,
                named_values: &[("PL011", 0x0)],
            },
        ],
    },
    AuthoredRegister {
        name: "PERIPHID2_REG",
        fields: &[
            AuthoredField {
                name: "REVISION",
                offset: 4,
                width: 4,
                named_values: &[
                    ("R1P0", 0x0),
                    ("R1P1", 0x1),
                    ("R1P3", 0x2),
                    ("R1P4", 0x2),
                    ("R1P5", 0x3),
                ],
            },
            AuthoredField {
                name: "DESIGNER1",
                offset: 0,
                width: 4,
                named_values: &[("ARM_HIGH", 0x4)],
            },
        ],
    },
    AuthoredRegister {
        name: "PERIPHID3_REG",
        fields: &[AuthoredField {
            name: "CONFIGURATION",
            offset: 0,
            width: 8,
            named_values: &[("DEFAULT", 0x00)],
        }],
    },
    AuthoredRegister {
        name: "PCELLID0_REG",
        fields: &[AuthoredField {
            name: "VALUE",
            offset: 0,
            width: 8,
            named_values: &[("DEFAULT", 0x0D)],
        }],
    },
    AuthoredRegister {
        name: "PCELLID1_REG",
        fields: &[AuthoredField {
            name: "VALUE",
            offset: 0,
            width: 8,
            named_values: &[("DEFAULT", 0xF0)],
        }],
    },
    AuthoredRegister {
        name: "PCELLID2_REG",
        fields: &[AuthoredField {
            name: "VALUE",
            offset: 0,
            width: 8,
            named_values: &[("DEFAULT", 0x05)],
        }],
    },
    AuthoredRegister {
        name: "PCELLID3_REG",
        fields: &[AuthoredField {
            name: "VALUE",
            offset: 0,
            width: 8,
            named_values: &[("DEFAULT", 0xB1)],
        }],
    },
];

// ---------------------------------------------------------------------------
// Inline specimen generator
//
// Generates Rust source matching the register_bitfields! macro expansion
// for all 22 PL011 registers. The expansion follows the same pattern as
// the regseq-macros register_bitfields macro: for each register, a module
// containing `struct Register;` and field sub-modules with `FIELD` constants
// (Field::new(mask, shift)) and optional named-value constants
// (FieldValue::new(mask, value)).
// ---------------------------------------------------------------------------

fn compute_mask(width: u32, offset: u32) -> u32 {
    ((1u32 << width) - 1) << offset
}

fn build_specimen_source() -> String {
    let mut modules = String::new();

    for reg in AUTHORED_REGISTERS {
        modules.push_str(&format!(
            "    #[allow(non_snake_case)]\n    \
             #[allow(non_upper_case_globals)]\n    \
             pub mod {} {{\n        pub struct Register;\n",
            reg.name
        ));

        for field in reg.fields {
            let mask = compute_mask(field.width, field.offset);
            let shift = field.offset;

            modules.push_str(&format!(
                "        #[allow(non_snake_case)]\n        \
                 #[allow(non_upper_case_globals)]\n        \
                 pub mod {} {{\n",
                field.name
            ));
            modules.push_str(&format!(
                "            pub const FIELD: super::super::Field<u32, super::Register> =\n                \
                 super::super::Field::new({mask} as u32, {shift} as u32);\n",
            ));

            for (nv_name, nv_value) in field.named_values {
                let shifted_value = nv_value << field.offset;
                modules.push_str(&format!(
                    "            pub const {nv_name}: \
                     super::super::FieldValue<u32, super::Register> =\n                \
                     super::super::FieldValue::new({mask} as u32, {shifted_value} as u32);\n",
                ));
            }

            modules.push_str("        }\n");
        }

        modules.push_str("    }\n\n");
    }

    format!("{}\n{}", FIXTURE_PREAMBLE, modules)
}

// ---------------------------------------------------------------------------
// Precondition: specimen fidelity verification against fixture source
// ---------------------------------------------------------------------------

/// Verify that AUTHORED_REGISTERS faithfully transcribes every register,
/// field, offset, width, and named value from the real PL011 fixture source.
#[test]
fn precondition_specimen_fidelity_against_fixture_source() {
    let fixture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pl011-full-spec/src/registers.rs"
    );
    let fixture_src =
        std::fs::read_to_string(fixture_path).expect("fixture source must be readable");

    // Verify every register name appears in the fixture source as a
    // register_bitfields! entry.
    for reg in AUTHORED_REGISTERS {
        assert!(
            fixture_src.contains(&format!("{} [", reg.name)),
            "fixture source must contain register definition for '{}'",
            reg.name
        );
    }

    // Verify total register count: the fixture defines exactly 22 registers.
    let fixture_reg_count = fixture_src
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.ends_with("_REG [")
        })
        .count();
    assert_eq!(
        fixture_reg_count,
        AUTHORED_REGISTERS.len(),
        "authored register count must match fixture register count"
    );

    // Verify every field appears in the fixture under its register with
    // the correct OFFSET and NUMBITS values.
    for reg in AUTHORED_REGISTERS {
        for field in reg.fields {
            let field_pattern = format!(
                "{} OFFSET({}) NUMBITS({})",
                field.name, field.offset, field.width
            );
            assert!(
                fixture_src.contains(&field_pattern),
                "fixture source must contain '{}' for register '{}'",
                field_pattern,
                reg.name
            );
        }
    }

    // Verify every named value appears in the fixture source within
    // the scope of its register definition.
    for reg in AUTHORED_REGISTERS {
        // Locate this register's section in the fixture source.
        // Use "\n    NAME [" to match whole-word register names at the
        // top-level indent (4 spaces), avoiding substring matches like
        // "RSR_ECR_REG" matching "CR_REG".
        let search_pattern = format!("\n    {} [", reg.name);
        let reg_start = fixture_src
            .find(&search_pattern)
            .unwrap_or_else(|| panic!("could not find register section for '{}'", reg.name));
        let reg_section = &fixture_src[reg_start..];
        // Top-level register closings use 4-space indent ("    ],") while
        // inner field closings use 8+ spaces. Skip past the first char
        // (which is the newline we matched) to find the next closing bracket.
        let reg_end = reg_section[1..]
            .find("\n    ]")
            .map(|i| i + 1)
            .unwrap_or(reg_section.len());
        let reg_text = &reg_section[..reg_end];

        for field in reg.fields {
            for (nv_name, nv_value) in field.named_values {
                // Verify the named value exists with correct numeric value.
                let found_value = reg_text.lines().find_map(|line| {
                    let trimmed = line.trim().trim_end_matches(',');
                    if trimmed.starts_with(&format!("{} = ", nv_name)) {
                        let val_str = trimmed.strip_prefix(&format!("{} = ", nv_name)).unwrap();
                        parse_rust_integer(val_str)
                    } else {
                        None
                    }
                });
                assert_eq!(
                    found_value,
                    Some(*nv_value),
                    "named value '{}.{}.{}' must have value {} in fixture",
                    reg.name,
                    field.name,
                    nv_name,
                    nv_value
                );
            }
        }
    }
}

/// Parse a Rust integer literal (decimal, binary 0b, hex 0x).
fn parse_rust_integer(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("0b") {
        u32::from_str_radix(rest, 2).ok()
    } else if let Some(rest) = s.strip_prefix("0x") {
        u32::from_str_radix(rest, 16).ok()
    } else {
        s.parse::<u32>().ok()
    }
}

// ---------------------------------------------------------------------------
// Check Group 1: register identity preservation
// ---------------------------------------------------------------------------

#[test]
fn check_group_1_register_identity_preserved() {
    let source = build_specimen_source();
    let surface = compile_and_extract(&source, "ste02_cg1_identity").unwrap();

    // Every authored register name must appear in register_descriptors.
    for reg in AUTHORED_REGISTERS {
        let found = surface
            .register_descriptors
            .iter()
            .find(|d| d.register_module == reg.name);
        assert!(
            found.is_some(),
            "register '{}' must be identifiable in extraction by its authored name",
            reg.name
        );
    }

    // Closed-world: total descriptor count must equal authored count.
    // This catches extra non-authored descriptors that filtering would miss.
    assert_eq!(
        surface.register_descriptors.len(),
        AUTHORED_REGISTERS.len(),
        "extraction must contain exactly {} register descriptors (no extras), got {}",
        AUTHORED_REGISTERS.len(),
        surface.register_descriptors.len()
    );
}

// ---------------------------------------------------------------------------
// Check Group 2: field inventory complete and in authored order
// ---------------------------------------------------------------------------

#[test]
fn check_group_2_field_inventory_complete_and_ordered() {
    let source = build_specimen_source();
    let surface = compile_and_extract(&source, "ste02_cg2_fields").unwrap();

    for reg in AUTHORED_REGISTERS {
        let descriptor = surface
            .register_descriptors
            .iter()
            .find(|d| d.register_module == reg.name)
            .unwrap_or_else(|| panic!("register '{}' must be present", reg.name));

        // Field count must match exactly (closed-world: no insertions or
        // removals).
        assert_eq!(
            descriptor.fields.len(),
            reg.fields.len(),
            "register '{}': extracted field count ({}) must equal authored count ({})",
            reg.name,
            descriptor.fields.len(),
            reg.fields.len()
        );

        // Field names must appear in authored order.
        let extracted_names: Vec<&str> = descriptor
            .fields
            .iter()
            .map(|f| f.field_name.as_str())
            .collect();
        let authored_names: Vec<&str> = reg.fields.iter().map(|f| f.name).collect();
        assert_eq!(
            extracted_names, authored_names,
            "register '{}': extracted field order {:?} must match authored order {:?}",
            reg.name, extracted_names, authored_names
        );
    }
}

// ---------------------------------------------------------------------------
// Check Group 3: field name, bit offset, and bit width preservation
// ---------------------------------------------------------------------------

#[test]
fn check_group_3_field_name_offset_width_preserved() {
    let source = build_specimen_source();
    let surface = compile_and_extract(&source, "ste02_cg3_props").unwrap();

    for reg in AUTHORED_REGISTERS {
        let descriptor = surface
            .register_descriptors
            .iter()
            .find(|d| d.register_module == reg.name)
            .unwrap_or_else(|| panic!("register '{}' must be present", reg.name));

        for authored_field in reg.fields {
            let extracted_field = descriptor
                .fields
                .iter()
                .find(|f| f.field_name == authored_field.name)
                .unwrap_or_else(|| {
                    panic!(
                        "register '{}': field '{}' must be present in extraction",
                        reg.name, authored_field.name
                    )
                });

            assert_eq!(
                extracted_field.offset, authored_field.offset,
                "register '{}' field '{}': offset {} must equal authored {}",
                reg.name, authored_field.name, extracted_field.offset, authored_field.offset
            );

            assert_eq!(
                extracted_field.width, authored_field.width,
                "register '{}' field '{}': width {} must equal authored {}",
                reg.name, authored_field.name, extracted_field.width, authored_field.width
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Check Group 4: named field value preservation
// ---------------------------------------------------------------------------

#[test]
fn check_group_4_named_values_preserved() {
    let source = build_specimen_source();
    let surface = compile_and_extract(&source, "ste02_cg4_values").unwrap();

    for reg in AUTHORED_REGISTERS {
        let descriptor = surface
            .register_descriptors
            .iter()
            .find(|d| d.register_module == reg.name)
            .unwrap_or_else(|| panic!("register '{}' must be present", reg.name));

        for authored_field in reg.fields {
            if authored_field.named_values.is_empty() {
                continue;
            }

            let extracted_field = descriptor
                .fields
                .iter()
                .find(|f| f.field_name == authored_field.name)
                .unwrap_or_else(|| {
                    panic!(
                        "register '{}': field '{}' must be present",
                        reg.name, authored_field.name
                    )
                });

            // Named value count must match exactly (closed-world).
            assert_eq!(
                extracted_field.named_values.len(),
                authored_field.named_values.len(),
                "register '{}' field '{}': named value count ({}) must equal authored count ({})",
                reg.name,
                authored_field.name,
                extracted_field.named_values.len(),
                authored_field.named_values.len()
            );

            // Each authored named value must be present with exact name and
            // value.
            for (nv_name, nv_value) in authored_field.named_values {
                let extracted_nv = extracted_field
                    .named_values
                    .iter()
                    .find(|v| v.name == *nv_name);
                assert!(
                    extracted_nv.is_some(),
                    "register '{}' field '{}': named value '{}' must be present",
                    reg.name,
                    authored_field.name,
                    nv_name
                );
                assert_eq!(
                    extracted_nv.unwrap().value,
                    *nv_value,
                    "register '{}' field '{}' value '{}': extracted {} must equal authored {}",
                    reg.name,
                    authored_field.name,
                    nv_name,
                    extracted_nv.unwrap().value,
                    nv_value
                );
            }

            // Named value order preservation.
            let extracted_nv_names: Vec<&str> = extracted_field
                .named_values
                .iter()
                .map(|v| v.name.as_str())
                .collect();
            let authored_nv_names: Vec<&str> = authored_field
                .named_values
                .iter()
                .map(|(n, _)| *n)
                .collect();
            assert_eq!(
                extracted_nv_names, authored_nv_names,
                "register '{}' field '{}': named value order {:?} must match authored {:?}",
                reg.name, authored_field.name, extracted_nv_names, authored_nv_names
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Check Group 5: no insertions, removals, renames, reorders, or changes
// (closed-world membership and ordering)
// ---------------------------------------------------------------------------

#[test]
fn check_group_5_exact_membership_and_ordering() {
    let source = build_specimen_source();
    let surface = compile_and_extract(&source, "ste02_cg5_closed").unwrap();

    // Closed-world: total descriptor count must equal authored count (no filtering).
    assert_eq!(
        surface.register_descriptors.len(),
        AUTHORED_REGISTERS.len(),
        "extraction must have exactly {} register descriptors (no extras), got {}",
        AUTHORED_REGISTERS.len(),
        surface.register_descriptors.len()
    );

    for reg in AUTHORED_REGISTERS {
        let descriptor = surface
            .register_descriptors
            .iter()
            .find(|d| d.register_module == reg.name)
            .unwrap_or_else(|| panic!("register '{}' must be present", reg.name));

        // Exact field count (no insertions or removals).
        assert_eq!(
            descriptor.fields.len(),
            reg.fields.len(),
            "register '{}': field count must be exactly {}",
            reg.name,
            reg.fields.len()
        );

        // Exact field names in authored order (no renames or reorders).
        let extracted_field_names: Vec<&str> = descriptor
            .fields
            .iter()
            .map(|f| f.field_name.as_str())
            .collect();
        let authored_field_names: Vec<&str> = reg.fields.iter().map(|f| f.name).collect();
        assert_eq!(
            extracted_field_names, authored_field_names,
            "register '{}': exact field membership and order required",
            reg.name
        );

        // Per-field: exact named value count, names, values, and order.
        for authored_field in reg.fields {
            let extracted_field = descriptor
                .fields
                .iter()
                .find(|f| f.field_name == authored_field.name)
                .unwrap();

            // Exact named value count.
            assert_eq!(
                extracted_field.named_values.len(),
                authored_field.named_values.len(),
                "register '{}' field '{}': named value count must be exactly {}",
                reg.name,
                authored_field.name,
                authored_field.named_values.len()
            );

            // Exact named value names, values, and order.
            let extracted_pairs: Vec<(&str, u32)> = extracted_field
                .named_values
                .iter()
                .map(|v| (v.name.as_str(), v.value))
                .collect();
            let authored_pairs: Vec<(&str, u32)> = authored_field
                .named_values
                .iter()
                .map(|(n, v)| (*n, *v))
                .collect();
            assert_eq!(
                extracted_pairs, authored_pairs,
                "register '{}' field '{}': exact named-value membership, values, and order required",
                reg.name, authored_field.name
            );
        }
    }
}
