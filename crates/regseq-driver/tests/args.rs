#![feature(rustc_private)]

extern crate rustc_driver;

use regseq_driver::args::{arg_value, has_arg, DispatchSignals, DriverArgs, DriverMode};

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn passthrough_signals() -> DispatchSignals {
    DispatchSignals {
        has_print_query: false,
        has_target: false,
        has_cargo_primary_package: false,
        has_regseq_options: false,
        current_pkg_name: None,
        regseq_aware_crates: None,
    }
}

#[test]
fn parse_wrapper_mode_extracts_rustc_path_and_args() {
    let parsed = DriverArgs::parse_from(&strings(&[
        "regseq-driver",
        "/usr/bin/rustc",
        "--crate-name",
        "foo",
    ]));
    assert_eq!(parsed.rustc_path.as_deref(), Some("/usr/bin/rustc"));
    assert_eq!(parsed.rustc_args, strings(&["--crate-name", "foo"]));
}

#[test]
fn parse_direct_invocation_leaves_rustc_path_empty() {
    let parsed = DriverArgs::parse_from(&strings(&["regseq-driver", "--crate-name", "foo"]));
    assert_eq!(parsed.rustc_path, None);
    assert_eq!(parsed.rustc_args, strings(&["--crate-name", "foo"]));
}

#[test]
fn mode_is_passthrough_when_all_signals_false() {
    let signals = passthrough_signals();
    assert_eq!(signals.mode(), DriverMode::Passthrough);
}

#[test]
fn mode_is_passthrough_for_print_queries() {
    let signals = DispatchSignals {
        has_print_query: true,
        has_target: true,
        has_cargo_primary_package: true,
        has_regseq_options: true,
        ..passthrough_signals()
    };
    assert_eq!(signals.mode(), DriverMode::Passthrough);
}

#[test]
fn mode_is_passthrough_without_target() {
    let signals = DispatchSignals {
        has_print_query: false,
        has_target: false,
        has_cargo_primary_package: true,
        has_regseq_options: true,
        ..passthrough_signals()
    };
    assert_eq!(signals.mode(), DriverMode::Passthrough);
}

#[test]
fn mode_is_passthrough_without_cargo_primary_package() {
    let signals = DispatchSignals {
        has_print_query: false,
        has_target: true,
        has_cargo_primary_package: false,
        has_regseq_options: true,
        ..passthrough_signals()
    };
    assert_eq!(signals.mode(), DriverMode::Passthrough);
}

#[test]
fn mode_is_passthrough_without_regseq_options() {
    let signals = DispatchSignals {
        has_print_query: false,
        has_target: true,
        has_cargo_primary_package: true,
        has_regseq_options: false,
        ..passthrough_signals()
    };
    assert_eq!(signals.mode(), DriverMode::Passthrough);
}

#[test]
fn mode_is_regseq_when_all_signals_present() {
    let signals = DispatchSignals {
        has_print_query: false,
        has_target: true,
        has_cargo_primary_package: true,
        has_regseq_options: true,
        ..passthrough_signals()
    };
    assert_eq!(signals.mode(), DriverMode::Regseq);
}

#[test]
fn from_rustc_args_detects_print_query_equals_form() {
    let signals = DispatchSignals::from_rustc_args(
        &strings(&["--print=sysroot"]),
        true,
        true,
        None,
        None,
    );
    assert!(signals.has_print_query);
}

#[test]
fn from_rustc_args_detects_print_query_vv_flag() {
    let signals = DispatchSignals::from_rustc_args(&strings(&["-vV"]), true, true, None, None);
    assert!(signals.has_print_query);
}

#[test]
fn from_rustc_args_detects_target_equals_form() {
    let signals = DispatchSignals::from_rustc_args(
        &strings(&["--target=x86_64-unknown-linux-gnu"]),
        true,
        true,
        None,
        None,
    );
    assert!(signals.has_target);
}

#[test]
fn from_rustc_args_detects_target_space_form() {
    let signals = DispatchSignals::from_rustc_args(
        &strings(&["--target", "x86_64-unknown-linux-gnu"]),
        true,
        true,
        None,
        None,
    );
    assert!(signals.has_target);
}

#[test]
fn has_arg_finds_present_flag() {
    let args = strings(&["--crate-name", "foo", "--target=x86_64-unknown-linux-gnu"]);
    assert!(has_arg(&args, "--target"));
}

#[test]
fn has_arg_returns_false_when_absent() {
    let args = strings(&["--crate-name", "foo"]);
    assert!(!has_arg(&args, "--target"));
}

#[test]
fn arg_value_reads_equals_form() {
    let args = strings(&["--target=x86_64-unknown-linux-gnu"]);
    assert_eq!(
        arg_value(&args, "--target"),
        Some("x86_64-unknown-linux-gnu")
    );
}

#[test]
fn arg_value_reads_space_form() {
    let args = strings(&["--target", "x86_64-unknown-linux-gnu"]);
    assert_eq!(
        arg_value(&args, "--target"),
        Some("x86_64-unknown-linux-gnu")
    );
}

#[test]
fn arg_value_returns_none_when_absent() {
    let args = strings(&["--crate-name", "foo"]);
    assert_eq!(arg_value(&args, "--target"), None);
}

// --- RegseqAware dispatch coverage ---

#[test]
fn mode_is_passthrough_for_aware_crate_without_env_var() {
    // Aware crate built outside cargo-regseq: REGSEQ_AWARE_CRATES is absent,
    // so it must fall through to Passthrough.
    let signals = DispatchSignals {
        has_print_query: false,
        has_target: true,
        has_cargo_primary_package: false,
        has_regseq_options: false,
        current_pkg_name: Some("regseq".to_string()),
        regseq_aware_crates: None,
    };
    assert_eq!(signals.mode(), DriverMode::Passthrough);
}

#[test]
fn mode_is_aware_when_pkg_name_matches_aware_list() {
    let signals = DispatchSignals {
        has_print_query: false,
        has_target: true,
        has_cargo_primary_package: false,
        has_regseq_options: false,
        current_pkg_name: Some("regseq".to_string()),
        regseq_aware_crates: Some(vec!["regseq".to_string()]),
    };
    assert_eq!(signals.mode(), DriverMode::RegseqAware);
}

#[test]
fn mode_is_passthrough_when_pkg_name_not_in_aware_list() {
    let signals = DispatchSignals {
        has_print_query: false,
        has_target: true,
        has_cargo_primary_package: false,
        has_regseq_options: false,
        current_pkg_name: Some("serde".to_string()),
        regseq_aware_crates: Some(vec!["regseq".to_string()]),
    };
    assert_eq!(signals.mode(), DriverMode::Passthrough);
}

#[test]
fn mode_prefers_regseq_over_aware_when_both_apply() {
    // A spec crate that has CARGO_PRIMARY_PACKAGE + REGSEQ_OPTIONS and
    // is also listed in REGSEQ_AWARE_CRATES (defensive case) must still
    // resolve to Regseq, not RegseqAware.
    let signals = DispatchSignals {
        has_print_query: false,
        has_target: true,
        has_cargo_primary_package: true,
        has_regseq_options: true,
        current_pkg_name: Some("pl011_spec".to_string()),
        regseq_aware_crates: Some(vec!["pl011_spec".to_string(), "regseq".to_string()]),
    };
    assert_eq!(signals.mode(), DriverMode::Regseq);
}

#[test]
fn mode_is_passthrough_for_print_query_even_if_aware() {
    let signals = DispatchSignals {
        has_print_query: true,
        has_target: true,
        has_cargo_primary_package: false,
        has_regseq_options: false,
        current_pkg_name: Some("regseq".to_string()),
        regseq_aware_crates: Some(vec!["regseq".to_string()]),
    };
    assert_eq!(signals.mode(), DriverMode::Passthrough);
}

#[test]
fn mode_is_passthrough_without_target_even_if_aware() {
    let signals = DispatchSignals {
        has_print_query: false,
        has_target: false,
        has_cargo_primary_package: false,
        has_regseq_options: false,
        current_pkg_name: Some("regseq".to_string()),
        regseq_aware_crates: Some(vec!["regseq".to_string()]),
    };
    assert_eq!(signals.mode(), DriverMode::Passthrough);
}

#[test]
fn mode_is_passthrough_when_aware_list_empty() {
    let signals = DispatchSignals {
        has_print_query: false,
        has_target: true,
        has_cargo_primary_package: false,
        has_regseq_options: false,
        current_pkg_name: Some("regseq".to_string()),
        regseq_aware_crates: Some(vec![]),
    };
    assert_eq!(signals.mode(), DriverMode::Passthrough);
}
