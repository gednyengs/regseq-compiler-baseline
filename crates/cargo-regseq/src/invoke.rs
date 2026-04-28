use std::process::{Command, Stdio};

use cargo_metadata::{Metadata, MetadataCommand};
use regseq_config::{ColorChoice, CompilerOptions};

use crate::toolchain::ToolchainInfo;
use crate::Error;

/// Build the `cargo check` command used to run Regseq compilation.
pub fn build_cargo_command(
    options: &CompilerOptions,
    toolchain: &ToolchainInfo,
    cargo_args: &[String],
    color: ColorChoice,
) -> Result<Command, Error> {
    let options_json = serde_json::to_string(options).map_err(|error| {
        Error::Invoke(format!(
            "failed to serialize CompilerOptions to JSON: {error}"
        ))
    })?;

    let aware_crates = collect_regseq_aware_crates(None)?;

    let mut command = Command::new("cargo");
    command.arg("check");
    command.env_remove("CARGO_PRIMARY_PACKAGE");
    command.env("RUSTC_WRAPPER", &toolchain.driver);
    command.env("REGSEQ_OPTIONS", options_json);
    command.env("REGSEQ_SYSROOT", &toolchain.sysroot);
    command.env("REGSEQ_AWARE_CRATES", aware_crates.join(","));

    if !cargo_args_contain_color(cargo_args) {
        command.arg("--color");
        command.arg(color.to_string());
    }

    if !cargo_args_contain_target(cargo_args) {
        command.arg("--target");
        command.arg(&toolchain.host);
    }

    command.args(cargo_args);
    Ok(command)
}

/// Invoke cargo with Regseq wrapper environment configured.
pub fn invoke_cargo(
    options: &CompilerOptions,
    toolchain: &ToolchainInfo,
    cargo_args: &[String],
    color: ColorChoice,
) -> Result<(), Error> {
    std::fs::create_dir_all(&options.output_dir)?;

    let mut command = build_cargo_command(options, toolchain, cargo_args, color)?;
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    let status = command
        .status()
        .map_err(|error| Error::Invoke(format!("failed to launch cargo: {error}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::Invoke(format!(
            "cargo check failed with status {status}"
        )))
    }
}

/// Run `cargo metadata` for the workspace at `manifest_path` (or the
/// current directory when `None`) and collect the names of every
/// package that declares a `[package.metadata.regseq]` table. The
/// order follows the order returned by `cargo metadata`, with no
/// deduplication beyond what cargo already produces.
fn collect_regseq_aware_crates(
    manifest_path: Option<&std::path::Path>,
) -> Result<Vec<String>, Error> {
    let mut command = MetadataCommand::new();
    if let Some(path) = manifest_path {
        command.manifest_path(path);
    }
    let metadata = command
        .exec()
        .map_err(|error| Error::Invoke(format!("cargo metadata failed: {error}")))?;
    Ok(extract_regseq_aware_names(&metadata))
}

/// Walk a parsed `cargo metadata` result and return the names of every
/// package whose `package.metadata.regseq` table is present (any
/// non-null JSON value under the `regseq` key, including an empty
/// object). The `role` field is ignored -- presence of the table is
/// the marker.
pub(crate) fn extract_regseq_aware_names(metadata: &Metadata) -> Vec<String> {
    let mut names = Vec::new();
    for package in &metadata.packages {
        let Some(meta) = package.metadata.as_object() else {
            continue;
        };
        let Some(value) = meta.get("regseq") else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        names.push(package.name.to_string());
    }
    names
}

fn cargo_args_contain_color(cargo_args: &[String]) -> bool {
    cargo_args
        .iter()
        .any(|arg| arg == "--color" || arg.starts_with("--color="))
}

fn cargo_args_contain_target(cargo_args: &[String]) -> bool {
    cargo_args
        .iter()
        .any(|arg| arg == "--target" || arg.starts_with("--target="))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargo_metadata::Metadata;

    fn metadata_from_json(json: &str) -> Metadata {
        serde_json::from_str(json).expect("synthetic metadata should deserialize")
    }

    fn pkg_json(name: &str, regseq_value: Option<&str>) -> String {
        let metadata_field = match regseq_value {
            Some(value) => format!(r#""metadata":{{"regseq":{value}}}"#),
            None => r#""metadata":null"#.to_string(),
        };
        format!(
            r#"{{
                "name":"{name}",
                "version":"0.1.0",
                "id":"path+file:///{name}#0.1.0",
                "license":null,
                "license_file":null,
                "description":null,
                "source":null,
                "dependencies":[],
                "targets":[],
                "features":{{}},
                "manifest_path":"/tmp/{name}/Cargo.toml",
                "categories":[],
                "keywords":[],
                "readme":null,
                "repository":null,
                "homepage":null,
                "documentation":null,
                "edition":"2021",
                "links":null,
                "publish":null,
                "default_run":null,
                "rust_version":null,
                "authors":[],
                {metadata_field}
            }}"#
        )
    }

    fn metadata_with_packages(pkgs: Vec<String>) -> Metadata {
        let pkgs_json = pkgs.join(",");
        let json = format!(
            r#"{{
                "packages":[{pkgs_json}],
                "workspace_members":[],
                "workspace_default_members":[],
                "resolve":null,
                "target_directory":"/tmp/target",
                "version":1,
                "workspace_root":"/tmp",
                "metadata":null
            }}"#
        );
        metadata_from_json(&json)
    }

    #[test]
    fn extract_regseq_aware_names_returns_empty_when_no_aware_crates() {
        let metadata = metadata_with_packages(vec![pkg_json("foo", None), pkg_json("bar", None)]);
        let names = extract_regseq_aware_names(&metadata);
        assert!(names.is_empty(), "expected empty, got {names:?}");
    }

    #[test]
    fn extract_regseq_aware_names_finds_single_aware_crate() {
        let metadata = metadata_with_packages(vec![
            pkg_json("foo", None),
            pkg_json("regseq", Some(r#"{"role":"facade"}"#)),
            pkg_json("baz", None),
        ]);
        let names = extract_regseq_aware_names(&metadata);
        assert_eq!(names, vec!["regseq".to_string()]);
    }

    #[test]
    fn extract_regseq_aware_names_finds_multiple_aware_crates() {
        let metadata = metadata_with_packages(vec![
            pkg_json("foo", Some(r#"{"role":"facade"}"#)),
            pkg_json("bar", None),
            pkg_json("baz", Some("{}")),
        ]);
        let names = extract_regseq_aware_names(&metadata);
        assert_eq!(names, vec!["foo".to_string(), "baz".to_string()]);
    }

    #[test]
    fn extract_regseq_aware_names_treats_explicit_null_as_absent() {
        let metadata = metadata_with_packages(vec![pkg_json("foo", Some("null"))]);
        let names = extract_regseq_aware_names(&metadata);
        assert!(
            names.is_empty(),
            "explicit null should be skipped, got {names:?}"
        );
    }
}
