//! regseq-driver-94: lazily-interned core generic ADTs are classified
//! `is_generic` (bucket 3) via the shared `source_bridge_item_flags`
//! helper, so a driver that uses `core::result::Result` at two
//! monomorphizations bridges without `Bucket1MultipleInstances`.
//!
//! Drives the production-faithful bridge (`try_bridge_fixture_path`:
//! `mir_opt_level=0` + the regseq cleanup-pass whitelist + `panic=abort`)
//! over dual-monomorphization `Result<_, RegseqError>` snippets and
//! asserts:
//!
//!   1. A program using `Result` at two monomorphizations
//!      (`Result<i32, RegseqError>` helper, `?`-propagated into a
//!      `Result<(), RegseqError>` entry) bridges to `Ok` -- NOT a
//!      `Bucket1MultipleInstances` rejection -- and the model's `Result`
//!      Item carries `is_generic == true` with two `InstanceKind::Enum`
//!      monomorphizations (Test Plan 1).
//!   2. The same dual-mono program emits TWO distinct `Result` structs
//!      (the unit/err niche form, size 1, and the `i32`/err tagged form,
//!      size 8) with no name collision, `cc`-compiles, and runs with the
//!      expected exit: the Ok path returns 0 and the Err path returns the
//!      mapped `ETIMEDOUT` errno through the `regseq_main__regseq_exit`
//!      adapter (Test Plan 2).
//!   3. A lazily-interned NON-generic core ADT (`core::fmt::Error`) stays
//!      bucket 1: its `is_generic` resolves to `false` (the changed
//!      `ty.rs` push path is exercised, but the helper leaves a
//!      non-generic ADT at the default flag) (Test Plan 3, lazy-ADT half).
//!
//! The trait-assoc carve-out half of Test Plan 3 (a source trait-
//! declaration default method stays bucket 1 after the helper refactor)
//! lives with the eager-path classification tests in
//! `crates/regseq-driver/tests/bridge_eager_hir_enumeration.rs`.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::toolchain;
use regseq_backends::emit_generic_c_for_entry;
use regseq_model::{BridgeItemFlags, InstanceKind, ItemId, RegseqModel};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

/// A temp `.rs` file removed on drop (the `question_mark_e2e.rs` pattern).
struct TempSnippet {
    path: PathBuf,
}

impl TempSnippet {
    fn new(label: &str, src: &str) -> TempSnippet {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "ccr-coreadt-{label}-{}-{nanos}.rs",
            std::process::id()
        ));
        std::fs::write(&path, src).expect("must write temp snippet");
        TempSnippet { path }
    }
}

impl Drop for TempSnippet {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Bridge an inline fixture that must succeed, returning the model.
fn bridge_ok(label: &str, src: &str) -> RegseqModel {
    let snippet = TempSnippet::new(label, src);
    match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("fixture {label:?} must bridge to Ok, got {other:?}"),
    }
}

/// The `ItemId` of the `core::result::Result` Item (identified by the
/// `is_result` nominal flag the bridge sets via `SpecialAdt::Result`).
fn result_item_id(model: &RegseqModel) -> ItemId {
    for (idx, item) in model.items.iter().enumerate() {
        if item.is_result {
            return ItemId::from_u32(idx as u32);
        }
    }
    panic!("no `core::result::Result` Item (is_result flag) in the model");
}

/// The `ItemId` of the source Item whose rendered `DefPath` ends with
/// `suffix` (e.g. `"::fmt::Error"`).
fn item_id_by_suffix(model: &RegseqModel, suffix: &str) -> ItemId {
    for (idx, item) in model.items.iter().enumerate() {
        if model
            .strings
            .resolve(item.def_path.rendered)
            .ends_with(suffix)
        {
            return ItemId::from_u32(idx as u32);
        }
    }
    panic!(
        "no Item whose rendered DefPath ends with {suffix:?}; items: {:?}",
        model
            .items
            .iter()
            .map(|it| model.strings.resolve(it.def_path.rendered))
            .collect::<Vec<_>>()
    );
}

/// The four-bucket flags for `id`, falling back to the default
/// (`is_generic == false`) on a sparse-table miss (the validator's own
/// fallback).
fn flags(model: &RegseqModel, id: ItemId) -> BridgeItemFlags {
    model
        .item_bridge_flags
        .get(&id)
        .copied()
        .unwrap_or_default()
}

/// Count the `InstanceKind::Enum` monomorphizations attached to `id`.
fn enum_instances(model: &RegseqModel, id: ItemId) -> usize {
    model
        .instances
        .iter()
        .filter(|inst| inst.item == id && matches!(inst.kind, InstanceKind::Enum { .. }))
        .count()
}

/// Collect `(name, size)` for every emitted `core_result_Result__<hex>`
/// struct from its `_Static_assert(sizeof(...) == N, ...)` line.
fn result_structs(tu: &str) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    for line in tu.lines() {
        let Some(rest) = line.trim().strip_prefix("_Static_assert(sizeof(struct ") else {
            continue;
        };
        let Some((name, tail)) = rest.split_once(')') else {
            continue;
        };
        if !name.starts_with("core_result_Result__") {
            continue;
        }
        let Some(after_eq) = tail.trim_start().strip_prefix("== ") else {
            continue;
        };
        let Some((digits, _)) = after_eq.split_once(',') else {
            continue;
        };
        if let Ok(size) = digits.trim().parse::<u64>() {
            out.push((name.to_string(), size));
        }
    }
    out
}

/// A `#[rseq::framework_type("regseq-error")]` driver that uses `Result`
/// at two monomorphizations: a helper returning `Result<i32, RegseqError>`
/// `?`-propagated (the same-error-type normalizer, regseq-driver-92) into
/// the `Result<(), RegseqError>` entry. `probe_val(0)` flows the `Ok(7)`
/// payload through the `?` and the entry returns `Ok(())` (exit 0).
const DUAL_MONO_OK_SRC: &str = r#"#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::framework_type("regseq-error")]
pub enum RegseqError {
    DeviceBusy,
    Timeout,
}

fn probe_val(code: u8) -> Result<i32, RegseqError> {
    if code == 0 {
        Ok(7)
    } else {
        Err(RegseqError::DeviceBusy)
    }
}

pub fn regseq_main() -> Result<(), RegseqError> {
    let _v = probe_val(0)?;
    Ok(())
}
"#;

/// The Err-path sibling of [`DUAL_MONO_OK_SRC`]: `probe_val(1)` returns
/// `Err(RegseqError::Timeout)`, which `?` propagates so the entry returns
/// `Err(Timeout)` -- mapped by the adapter to the `ETIMEDOUT` errno.
const DUAL_MONO_ERR_SRC: &str = r#"#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::framework_type("regseq-error")]
pub enum RegseqError {
    DeviceBusy,
    Timeout,
}

fn probe_val(code: u8) -> Result<i32, RegseqError> {
    if code == 0 {
        Ok(7)
    } else {
        Err(RegseqError::Timeout)
    }
}

pub fn regseq_main() -> Result<(), RegseqError> {
    let _v = probe_val(1)?;
    Ok(())
}
"#;

// ---------------------------------------------------------------------------
// Test Plan 1 -- dual-mono `Result` bridges (no `Bucket1MultipleInstances`)
// and the `Result` Item is classified `is_generic`.
// ---------------------------------------------------------------------------

#[test]
fn dual_mono_result_bridges_and_is_generic() {
    let model = bridge_ok("dual-ok", DUAL_MONO_OK_SRC);

    let result_id = result_item_id(&model);
    assert!(
        flags(&model, result_id).is_generic,
        "lazily-interned core `Result` must be classified is_generic == true (bucket 3)"
    );
    assert_eq!(
        enum_instances(&model, result_id),
        2,
        "the dual-mono program must produce exactly two `Result` enum monomorphizations \
         (Result<i32, RegseqError> + Result<(), RegseqError>)"
    );
}

// ---------------------------------------------------------------------------
// Test Plan 2 -- two distinct `Result` structs, cc + run with Ok -> 0 and
// Err -> the mapped errno.
// ---------------------------------------------------------------------------

#[test]
fn dual_mono_emits_two_distinct_result_structs_and_runs_ok_path() {
    let model = bridge_ok("dual-ok-emit", DUAL_MONO_OK_SRC);
    let emit = emit_generic_c_for_entry(&model, "regseq_main")
        .expect("the dual-mono Result driver must emit for its entry");

    let mut structs = result_structs(&emit.tu);
    structs.sort();
    structs.dedup();
    assert_eq!(
        structs.len(),
        2,
        "expected two distinct emitted `Result` structs (no name collision), got {structs:?}"
    );
    assert_ne!(
        structs[0].0, structs[1].0,
        "the two per-mono `Result` structs must have distinct C names: {structs:?}"
    );
    let sizes: Vec<u64> = {
        let mut s: Vec<u64> = structs.iter().map(|(_, size)| *size).collect();
        s.sort_unstable();
        s
    };
    assert_eq!(
        sizes,
        vec![1, 8],
        "the unit/err `Result` must be size 1 (niche) and the i32/err `Result` size 8 (tagged); \
         got {structs:?}"
    );

    // Ok path: `probe_val(0)` -> `Ok(7)` -> entry returns `Ok(())` -> exit 0.
    let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc())
        .expect("the dual-mono Result TU must compile + run");
    assert_eq!(
        run.exit_code,
        Some(0),
        "the Ok-path dual-mono entry must exit 0; stderr: {}",
        run.stderr
    );
    assert!(
        run.stderr.is_empty(),
        "the Ok-path run must have empty stderr; stderr: {}",
        run.stderr
    );
}

#[test]
fn dual_mono_err_path_runs_with_mapped_errno() {
    let model = bridge_ok("dual-err-emit", DUAL_MONO_ERR_SRC);
    let emit = emit_generic_c_for_entry(&model, "regseq_main")
        .expect("the dual-mono Err Result driver must emit for its entry");

    // Two distinct `Result` structs are emitted on the Err path too.
    let mut structs = result_structs(&emit.tu);
    structs.sort();
    structs.dedup();
    assert_eq!(
        structs.len(),
        2,
        "the Err-path dual-mono program must also emit two distinct `Result` structs, got {structs:?}"
    );

    // Err path: `probe_val(1)` -> `Err(Timeout)` -> entry returns
    // `Err(Timeout)` -> adapter maps it to `ETIMEDOUT`.
    let expected = toolchain::resolve_errno("ETIMEDOUT", cc())
        .expect("ETIMEDOUT must resolve on the host libc");
    let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc())
        .expect("the dual-mono Err Result TU must compile + run");
    assert_eq!(
        run.exit_code,
        Some(expected),
        "the Err-path dual-mono entry must exit with the mapped ETIMEDOUT errno ({expected}); \
         stderr: {}",
        run.stderr
    );
}

// ---------------------------------------------------------------------------
// Test Plan 3 (lazy-ADT half) -- a lazily-interned NON-generic core ADT
// stays bucket 1 (is_generic == false) through the changed `ty.rs` push.
// ---------------------------------------------------------------------------

#[test]
fn lazily_interned_non_generic_core_adt_stays_bucket1() {
    let model = bridge_ok(
        "lazy-nongeneric",
        r#"#![no_std]
fn make_err() -> core::fmt::Error {
    core::fmt::Error
}
#[no_mangle]
pub extern "C" fn regseq_main() {
    let _ = make_err();
}
"#,
    );

    // `core::fmt::Error` is a non-generic unit struct reached only
    // lazily (it is never in the source crate), so it exercises the
    // changed `intern_source_item_handle` push -- and the shared helper
    // must leave it at the default `is_generic == false` (bucket 1).
    let err_id = item_id_by_suffix(&model, "::fmt::Error");
    assert!(
        !flags(&model, err_id).is_generic,
        "a lazily-interned non-generic core ADT must stay is_generic == false (bucket 1)"
    );
}
