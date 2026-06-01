//! Emit dispatcher for `regseq-driver`. Wires `EmitSpec` variants to
//! their corresponding `RegseqModel` emission paths:
//! pretty-printed text via
//! `regseq_model::PrettyPrinter::new(&model).render()` per
//! `pretty-printer.md` section 10.3, JSON via
//! `serde_json::to_writer_pretty` over `RegseqModel`'s
//! `Serialize` derive at `crates/regseq-model/src/model.rs:53`, or
//! the generic-C translation unit via
//! `regseq_backends::emit_generic_c(&model)`.
//!
//! Per `pipeline-refactor.md:879`, the `Driver` and `Analysis`
//! variants are accepted at the CLI surface but not implemented
//! until phase-1b; this dispatcher returns `Ok(())` for them and
//! defers the user-facing warning to the caller (so this module
//! stays rustc-agnostic and unit-testable without a `TyCtxt`).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use regseq_config::EmitSpec;
use regseq_model::{PrettyPrinter, RegseqModel};

/// Dispatch a single `EmitSpec` against the validated model.
///
/// `output_dir` is the user-configured base directory (default
/// `./regseq-out/` per `crates/cargo-regseq/src/cli.rs:32`); the
/// per-spec `path` overrides the default file name when present.
///
/// Returns `Ok(())` for spec variants the dispatcher cannot yet
/// service (`Driver` and `Analysis`); callers are responsible for
/// surfacing user-visible warnings via
/// `collect_unsupported_warnings`.
pub fn emit_one(model: &RegseqModel, spec: &EmitSpec, output_dir: &Path) -> std::io::Result<()> {
    match spec {
        EmitSpec::Model { path } => {
            let dest = resolve(path, output_dir, "model.txt");
            ensure_parent(&dest)?;
            let rendered = PrettyPrinter::new(model).render();
            fs::write(dest, rendered)
        }
        EmitSpec::Json { path } => {
            let dest = resolve(path, output_dir, "model.json");
            ensure_parent(&dest)?;
            let mut file = fs::File::create(dest)?;
            serde_json::to_writer_pretty(&mut file, model)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            file.write_all(b"\n")
        }
        EmitSpec::GenericC { path } => {
            let dest = resolve(path, output_dir, "generic.c");
            ensure_parent(&dest)?;
            let code = regseq_backends::emit_generic_c(model)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            fs::write(dest, code)
        }
        EmitSpec::Bin { name: _ } => {
            // The driver produces the *source half* of the binary: the library
            // TU `generic.c` plus the entry shim `generic_main.c`, both at the
            // default location. `cargo-regseq` links them after `cargo check`
            // (the `name` is the linker's output name, not used here). The
            // backend is the only place that can resolve the crate-qualified
            // entry symbol from the model, so the shim is written here.
            let tu_dest = output_dir.join("generic.c");
            ensure_parent(&tu_dest)?;
            let code = regseq_backends::emit_generic_c(model)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            fs::write(&tu_dest, code)?;

            let entry = match regseq_backends::entry_c_symbol(model, "regseq_main") {
                regseq_backends::EntrySymbol::Resolved(sym) => sym,
                regseq_backends::EntrySymbol::NotFound => {
                    return Err(std::io::Error::other(
                        "--emit bin: no entry function found (expected a function whose \
                         emission name ends with `regseq_main`)",
                    ));
                }
                regseq_backends::EntrySymbol::Ambiguous(syms) => {
                    return Err(std::io::Error::other(format!(
                        "--emit bin: ambiguous entry -- {} functions match the `regseq_main` \
                         suffix ({})",
                        syms.len(),
                        syms.join(", ")
                    )));
                }
            };
            let main_dest = output_dir.join("generic_main.c");
            fs::write(main_dest, regseq_backends::generate_main_c(&entry))
        }
        EmitSpec::Driver { .. } | EmitSpec::Analysis { .. } => {
            // Phase-1b territory per `pipeline-refactor.md:879`.
            // The dispatcher returns `Ok(())` silently; the
            // user-facing warning lives in the caller
            // (`callbacks.rs::after_analysis`) via
            // `collect_unsupported_warnings` routed through
            // `tcx.dcx().warn`.
            Ok(())
        }
    }
}

/// Collect human-readable warning strings for emit specs the
/// dispatcher cannot service (`Driver` and `Analysis`).
///
/// The caller is responsible for routing these strings through
/// rustc's diagnostic system (`tcx.dcx().warn`). Separating
/// collection from emission keeps `emit.rs` rustc-agnostic and
/// unit-testable without a `TyCtxt`.
pub fn collect_unsupported_warnings(specs: &[EmitSpec]) -> Vec<String> {
    specs
        .iter()
        .filter_map(|spec| match spec {
            EmitSpec::Driver { .. } | EmitSpec::Analysis { .. } => Some(format!(
                "regseq-driver: --emit {spec} is documented but not yet implemented (phase-1b); skipping",
            )),
            EmitSpec::Model { .. }
            | EmitSpec::Json { .. }
            | EmitSpec::GenericC { .. }
            | EmitSpec::Bin { .. } => None,
        })
        .collect()
}

fn resolve(path: &Option<PathBuf>, output_dir: &Path, default_name: &str) -> PathBuf {
    path.clone()
        .unwrap_or_else(|| output_dir.join(default_name))
}

fn ensure_parent(dest: &Path) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_model::{RegseqModel, StableCrateId};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Serialises any unit test that mutates the process current
    /// directory (e.g. via `std::env::set_current_dir`). Cargo runs
    /// unit tests in parallel within a single test binary by default,
    /// so CWD mutations without a guard race against any concurrent
    /// test that observes CWD or uses a relative path. Mirrors the
    /// `COMPILER_LOCK` pattern at
    /// `crates/regseq-driver/tests/emit_dispatch.rs:28`.
    static CWD_MUTEX: Mutex<()> = Mutex::new(());

    fn small_model() -> RegseqModel {
        RegseqModel::with_primary("emit_test_crate", StableCrateId(7))
    }

    fn fresh_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is past UNIX epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("regseq-emit-{label}-{nanos}"))
    }

    #[test]
    fn emit_one_model_default_path_writes_pretty_print_to_output_dir() {
        let model = small_model();
        let dir = fresh_dir("model-default");
        emit_one(&model, &EmitSpec::Model { path: None }, &dir).expect("model emit must succeed");
        let dest = dir.join("model.txt");
        let contents = fs::read_to_string(&dest).expect("model.txt must exist");
        let expected = PrettyPrinter::new(&model).render();
        assert_eq!(contents, expected);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn emit_one_model_explicit_path_writes_to_that_path() {
        let model = small_model();
        let dir = fresh_dir("model-explicit");
        fs::create_dir_all(&dir).expect("mkdir");
        let explicit = dir.join("override.txt");
        emit_one(
            &model,
            &EmitSpec::Model {
                path: Some(explicit.clone()),
            },
            // output_dir is intentionally a different directory; the
            // explicit per-spec path must take precedence.
            &dir.join("ignored"),
        )
        .expect("emit must succeed");
        assert!(explicit.exists(), "explicit path must be created");
        assert!(
            !dir.join("ignored").join("model.txt").exists(),
            "default path must not be used when an explicit path is set"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn emit_one_generic_c_default_path_writes_to_output_dir() {
        let model = small_model();
        let dir = fresh_dir("generic-c-default");
        emit_one(&model, &EmitSpec::GenericC { path: None }, &dir)
            .expect("generic-c emit must succeed");
        let dest = dir.join("generic.c");
        let contents = fs::read_to_string(&dest).expect("generic.c must exist");
        let expected = regseq_backends::emit_generic_c(&model).expect("backend emit");
        assert_eq!(contents, expected);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn emit_one_bin_without_entry_writes_tu_then_errors() {
        // `small_model` has no functions, so entry resolution finds no
        // `regseq_main` -> `emit_one(Bin)` writes `generic.c` (the library
        // TU half) but then errors before the shim, surfacing as a fatal
        // driver diagnostic at the call site. (The resolved happy path is
        // covered end-to-end in `tests/generic_c_emit.rs`'s `--emit bin`
        // sibling, where a real crate defines `regseq_main`.)
        let model = small_model();
        let dir = fresh_dir("bin-no-entry");
        let err = emit_one(&model, &EmitSpec::Bin { name: None }, &dir)
            .expect_err("bin emit must fail when there is no entry");
        assert!(
            err.to_string().contains("no entry function"),
            "error must explain the missing entry: {err}"
        );
        // The TU half was still written; only the shim is missing.
        assert!(dir.join("generic.c").exists(), "generic.c must be written");
        assert!(
            !dir.join("generic_main.c").exists(),
            "no shim when the entry is unresolved"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn emit_one_generic_c_explicit_path_writes_to_that_path() {
        let model = small_model();
        let dir = fresh_dir("generic-c-explicit");
        fs::create_dir_all(&dir).expect("mkdir");
        let explicit = dir.join("driver.c");
        emit_one(
            &model,
            &EmitSpec::GenericC {
                path: Some(explicit.clone()),
            },
            // output_dir is intentionally a different directory; the
            // explicit per-spec path must take precedence.
            &dir.join("ignored"),
        )
        .expect("emit must succeed");
        assert!(explicit.exists(), "explicit path must be created");
        assert!(
            !dir.join("ignored").join("generic.c").exists(),
            "default path must not be used when an explicit path is set"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn emit_one_json_default_path_writes_deserializable_json() {
        let model = small_model();
        let dir = fresh_dir("json-default");
        emit_one(&model, &EmitSpec::Json { path: None }, &dir).expect("json emit must succeed");
        let dest = dir.join("model.json");
        let file = fs::File::open(&dest).expect("model.json must exist");
        let decoded: RegseqModel = serde_json::from_reader(file).expect("must deserialize");
        assert_eq!(decoded.crates.len(), model.crates.len());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn emit_one_driver_and_analysis_are_silent_no_ops() {
        let model = small_model();
        let dir = fresh_dir("noop");
        fs::create_dir_all(&dir).expect("mkdir");
        emit_one(&model, &EmitSpec::Driver { match_id: None }, &dir)
            .expect("driver dispatch must be a silent no-op");
        emit_one(
            &model,
            &EmitSpec::Analysis {
                analysis_name: None,
                path: None,
            },
            &dir,
        )
        .expect("analysis dispatch must be a silent no-op");
        // Nothing should be written by the no-op arms.
        assert!(fs::read_dir(&dir).unwrap().next().is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_unsupported_warnings_reports_driver_and_analysis_only() {
        let specs = vec![
            EmitSpec::Driver { match_id: None },
            EmitSpec::Analysis {
                analysis_name: None,
                path: None,
            },
            EmitSpec::Model { path: None },
            EmitSpec::Json { path: None },
        ];
        let warnings = collect_unsupported_warnings(&specs);
        assert_eq!(warnings.len(), 2);
        assert!(warnings.iter().all(|w| w.contains("phase-1b")));
        assert!(warnings.iter().all(|w| w.contains("skipping")));
        assert!(warnings.iter().any(|w| w.contains("driver")));
        assert!(warnings.iter().any(|w| w.contains("analysis")));
    }

    #[test]
    fn collect_unsupported_warnings_empty_for_model_and_json_only() {
        let specs = vec![
            EmitSpec::Model { path: None },
            EmitSpec::Json { path: None },
        ];
        assert!(collect_unsupported_warnings(&specs).is_empty());
    }

    #[test]
    fn emit_one_explicit_relative_path_in_cwd_does_not_panic() {
        // `Path::new("foo.txt").parent() == Some("")`; the helper
        // must treat that as "no directory to create" and write to
        // the current working directory successfully. We run this
        // in a temp dir to avoid polluting the project workspace.
        //
        // CWD_MUTEX serialises this test against any future
        // CWD-mutating sibling so the chdir window doesn't race with
        // another test observing CWD or using a relative path.
        let _guard = CWD_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = fresh_dir("relative-cwd");
        fs::create_dir_all(&dir).expect("mkdir");
        let prev_cwd = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(&dir).expect("chdir");
        let result = emit_one(
            &small_model(),
            &EmitSpec::Model {
                path: Some(PathBuf::from("foo.txt")),
            },
            // output_dir is irrelevant when an explicit path is set.
            Path::new("does-not-matter"),
        );
        std::env::set_current_dir(prev_cwd).expect("chdir back");
        result.expect("relative-cwd emit must succeed");
        assert!(dir.join("foo.txt").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn emit_one_nested_relative_path_creates_subdirs() {
        let dir = fresh_dir("nested");
        let nested = dir.join("nested/dir/foo.txt");
        emit_one(
            &small_model(),
            &EmitSpec::Model {
                path: Some(nested.clone()),
            },
            Path::new("ignored"),
        )
        .expect("nested emit must succeed");
        assert!(nested.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn emit_one_default_path_creates_output_dir_when_missing() {
        let dir = fresh_dir("missing-output-dir");
        let nested_output = dir.join("deeper/path");
        emit_one(
            &small_model(),
            &EmitSpec::Model { path: None },
            &nested_output,
        )
        .expect("emit with missing output_dir must succeed");
        assert!(nested_output.join("model.txt").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
