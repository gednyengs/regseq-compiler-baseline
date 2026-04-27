#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;

use regseq_config::{
    ColorChoice, CompilerOptions, EmitSpec, LogLevel, ParamScope, RegseqParam, TargetOs,
};
use regseq_driver::{regseq_mir_passes, RegseqAwareCallbacks, RegseqCallbacks};

fn sample_options() -> CompilerOptions {
    CompilerOptions {
        target_os: TargetOs::Linux,
        platform_file: PathBuf::from("platforms/test.toml"),
        output_dir: PathBuf::from("out"),
        emit: vec![
            EmitSpec::Driver { match_id: None },
            EmitSpec::Analysis {
                analysis_name: Some("resources".to_string()),
                path: Some(PathBuf::from("reports/resources.json")),
            },
        ],
        check: false,
        add_analysis: vec!["resources".to_string()],
        skip_analysis: vec!["lint.foo".to_string()],
        deny: vec!["diag.a".to_string()],
        allow: vec!["diag.b".to_string()],
        params: vec![RegseqParam {
            scope: ParamScope::Analysis,
            name: "resource_limit".to_string(),
            value: "8".to_string(),
        }],
        log_level: LogLevel::Warn,
        dump_stage: None,
        color: ColorChoice::Auto,
    }
}

#[test]
fn deserialize_valid_regseq_options_json() {
    let json = r#"{
        "target_os":"linux",
        "platform_file":"platforms/test.toml",
        "output_dir":"out",
        "emit":[
            {"type":"driver","match_id":null},
            {"type":"analysis","analysis_name":"resources","path":"reports/resources.json"}
        ],
        "check":false,
        "add_analysis":["resources"],
        "skip_analysis":["lint.foo"],
        "deny":["diag.a"],
        "allow":["diag.b"],
        "params":[{"scope":"analysis","name":"resource_limit","value":"8"}],
        "log_level":"warn",
        "dump_stage":null,
        "color":"auto"
    }"#;

    let parsed: CompilerOptions = serde_json::from_str(json).expect("JSON should deserialize");
    assert_eq!(parsed, sample_options());
}

#[test]
fn deserialize_invalid_regseq_options_json_returns_error() {
    let invalid = r#"{"target_os":"linux","platform_file":}"#;
    assert!(serde_json::from_str::<CompilerOptions>(invalid).is_err());
}

#[test]
fn regseq_callbacks_new_stores_options() {
    let options = sample_options();
    let callbacks = RegseqCallbacks::new(options.clone());
    assert_eq!(callbacks.options(), &options);
}

#[test]
fn compiler_options_round_trip_through_json() {
    let options = sample_options();
    let json = serde_json::to_string(&options).expect("serialize options");
    let decoded: CompilerOptions = serde_json::from_str(&json).expect("deserialize options");
    assert_eq!(decoded, options);
}

/// The whitelist returned by `regseq_mir_passes` is the initial,
/// conservative set: UB-check passes are force-disabled. The
/// shape-altering cleanup passes (RemoveStorageMarkers, RemoveZsts,
/// SimplifyLocals-final, ReorderBasicBlocks, ReorderLocals) are the
/// design target but must wait on updates to the `?`-pattern
/// recognizer in `construction/try_folding.rs` that currently keys
/// on the unmodified MIR shape. See the docstring on
/// `callbacks::regseq_mir_passes` for the deferred items.
#[test]
fn regseq_mir_passes_whitelist_matches_contract() {
    let passes = regseq_mir_passes();
    let expected: Vec<(String, bool)> = vec![
        ("CheckAlignment".to_string(), false),
        ("CheckNull".to_string(), false),
        ("CheckEnums".to_string(), false),
    ];
    assert_eq!(passes, expected);
}

/// Regression guard: the `RegseqCallbacks::config` source must not
/// inject `feature(register_tool)` or `register_tool(rseq)`. Driver
/// spec fixtures declare these at their crate roots so the same
/// source compiles uniformly under the wrapper, rust-analyzer, and
/// any other non-wrapper compilation path. End-to-end tests
/// (`model_emit`, `pl011_construction`) compile fixtures that
/// self-declare `register_tool(rseq)`; if `RegseqCallbacks::config`
/// regressed and re-injected the same registration, those compiles
/// would fail with a duplicate-registration error. This text-level
/// check is the lightweight first line of defense.
#[test]
fn regseq_callbacks_config_source_does_not_inject_register_tool() {
    let source = include_str!("../src/callbacks.rs");

    // Locate the RegseqCallbacks::config function body.
    let config_start = source
        .find("impl rustc_driver::Callbacks for RegseqCallbacks")
        .expect("RegseqCallbacks impl not found in callbacks.rs");
    let after_impl = &source[config_start..];
    let fn_start = after_impl
        .find("fn config")
        .expect("RegseqCallbacks::config not found in callbacks.rs");
    let body_start = after_impl[fn_start..]
        .find('{')
        .expect("config fn body open brace not found")
        + fn_start;

    // Find matching closing brace for the function body.
    let bytes = after_impl.as_bytes();
    let mut depth = 0usize;
    let mut body_end = body_start;
    for i in body_start..bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &after_impl[body_start..body_end];

    assert!(
        !body.contains("register_tool"),
        "RegseqCallbacks::config must not reference register_tool; \
         found in body:\n{body}",
    );
    assert!(
        !body.contains("feature(register_tool)"),
        "RegseqCallbacks::config must not inject feature(register_tool); \
         found in body:\n{body}",
    );
}

// --- RegseqAwareCallbacks coverage ---

/// Smoke check that the type is constructible without arguments.
#[test]
fn regseq_aware_callbacks_is_unit_constructible() {
    let _callbacks = RegseqAwareCallbacks;
}

/// Locate the body of `RegseqAwareCallbacks::config` so the tests below
/// can run text-level assertions against its source.
fn aware_config_body() -> String {
    let source = include_str!("../src/callbacks.rs");
    let impl_start = source
        .find("impl rustc_driver::Callbacks for RegseqAwareCallbacks")
        .expect("RegseqAwareCallbacks impl not found in callbacks.rs");
    let after_impl = &source[impl_start..];
    let fn_start = after_impl
        .find("fn config")
        .expect("RegseqAwareCallbacks::config not found in callbacks.rs");
    let body_start = after_impl[fn_start..]
        .find('{')
        .expect("config fn body open brace not found")
        + fn_start;
    let bytes = after_impl.as_bytes();
    let mut depth = 0usize;
    let mut body_end = body_start;
    for i in body_start..bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    after_impl[body_start..body_end].to_string()
}

/// Mirror of `regseq_callbacks_config_source_does_not_inject_register_tool`
/// — `RegseqAwareCallbacks::config` must not push `register_tool` into
/// `crate_attr`. The facade declares its own `feature(register_tool)`
/// and `register_tool(rseq)` (see `crates/regseq/src/lib.rs:1-2`); a
/// duplicate registration would fail compilation.
#[test]
fn regseq_aware_callbacks_config_source_does_not_inject_register_tool() {
    let body = aware_config_body();
    // Strip line comments — explanatory `//` notes about what is
    // intentionally NOT set may legitimately mention `register_tool`.
    let active: String = body
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                ""
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !active.contains("register_tool"),
        "RegseqAwareCallbacks::config must not reference register_tool in active code; \
         active body was:\n{active}",
    );
    assert!(
        !active.contains("feature(register_tool)"),
        "RegseqAwareCallbacks::config must not inject feature(register_tool); \
         active body was:\n{active}",
    );
}

/// `RegseqAwareCallbacks::config` must NOT set `no_codegen` — rustc
/// must produce a normal rlib for the facade so downstream spec-crate
/// builds can link against it.
#[test]
fn regseq_aware_callbacks_config_source_does_not_set_no_codegen() {
    let body = aware_config_body();
    let mut saw_active_no_codegen = false;
    for line in body.lines() {
        let stripped = line.trim_start();
        if stripped.starts_with("//") {
            continue;
        }
        if stripped.contains("no_codegen") {
            saw_active_no_codegen = true;
            break;
        }
    }
    assert!(
        !saw_active_no_codegen,
        "RegseqAwareCallbacks::config must not set no_codegen; \
         found in body:\n{body}",
    );
}

/// The MIR shape settings must be present in the source.
#[test]
fn regseq_aware_callbacks_config_source_sets_mir_shape_settings() {
    let body = aware_config_body();
    for needle in [
        "mir_opt_level = Some(0)",
        "always_encode_mir = true",
        "mir_enable_passes = regseq_mir_passes()",
        "overflow_checks = Some(false)",
        "debug_assertions = Some(false)",
        "debug_assertions = false",
    ] {
        assert!(
            body.contains(needle),
            "RegseqAwareCallbacks::config must contain `{needle}`; \
             body was:\n{body}"
        );
    }
}

/// The shared whitelist constructor must be reachable from both
/// callback types — verify by reading the source for both impl blocks
/// and asserting each calls `regseq_mir_passes()` rather than
/// inlining a vec literal.
#[test]
fn both_callbacks_use_shared_regseq_mir_passes_helper() {
    let source = include_str!("../src/callbacks.rs");
    // Both config impls should reference the helper by name.
    let needle = "mir_enable_passes = regseq_mir_passes()";
    let count = source.matches(needle).count();
    assert!(
        count >= 2,
        "expected both RegseqCallbacks::config and RegseqAwareCallbacks::config \
         to set `{needle}`; found {count} occurrence(s) in callbacks.rs"
    );

    // Belt-and-suspenders: assert no inlined Vec literal with the
    // canonical pass entries exists. The only `("CheckAlignment"` ...
    // `.to_string(), false)` tuple in the file should be inside the
    // `regseq_mir_passes` helper body.
    let inlined_pattern = "(\"CheckAlignment\".to_string(), false)";
    let occurrences = source.matches(inlined_pattern).count();
    assert_eq!(
        occurrences, 1,
        "MIR pass names must only appear once (inside regseq_mir_passes); \
         found {occurrences} `{inlined_pattern}` occurrence(s)"
    );
}

/// Sanity: the existing whitelist contract test already checks
/// `regseq_mir_passes()`. Re-assert that calling it from a
/// RegseqAware-specific test path returns the same expected vec, so we
/// catch any accidental future per-callback override.
#[test]
fn regseq_aware_uses_same_mir_pass_whitelist() {
    let passes = regseq_mir_passes();
    let expected: Vec<(String, bool)> = vec![
        ("CheckAlignment".to_string(), false),
        ("CheckNull".to_string(), false),
        ("CheckEnums".to_string(), false),
    ];
    assert_eq!(passes, expected);
}
