use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};

use cargo_regseq::toolchain::ToolchainInfo;
use cargo_regseq::Error;
use tempfile::NamedTempFile;

fn env_lock() -> &'static Mutex<()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvVarRestore {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarRestore {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        // SAFETY: Tests guard environment mutations with a global process-wide mutex.
        unsafe { std::env::set_var(key, value) };
        Self { key, previous }
    }

    fn remove(key: &'static str) -> Self {
        let previous = std::env::var_os(key);
        // SAFETY: Tests guard environment mutations with a global process-wide mutex.
        unsafe { std::env::remove_var(key) };
        Self { key, previous }
    }
}

impl Drop for EnvVarRestore {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => {
                // SAFETY: Tests guard environment mutations with a global process-wide mutex.
                unsafe { std::env::set_var(self.key, value) };
            }
            None => {
                // SAFETY: Tests guard environment mutations with a global process-wide mutex.
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }
}

#[test]
fn detect_reports_missing_driver_gracefully() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let _reset_driver = EnvVarRestore::remove("REGSEQ_DRIVER");

    match ToolchainInfo::detect() {
        Ok(info) => {
            assert!(info.rustc.is_file());
            assert!(info.sysroot.is_dir());
            assert!(info.driver.is_file());
        }
        Err(error) => {
            assert!(
                matches!(error, Error::Toolchain(message) if message.contains("regseq-driver"))
            );
        }
    }

    let rustc = std::process::Command::new("rustup")
        .args(["which", "rustc"])
        .output()
        .expect("run rustup which rustc");
    assert!(rustc.status.success());
    let rustc_path = String::from_utf8_lossy(&rustc.stdout).trim().to_string();
    assert!(!rustc_path.is_empty());
    assert!(std::path::Path::new(&rustc_path).is_file());

    let sysroot = std::process::Command::new(rustc_path)
        .args(["--print", "sysroot"])
        .output()
        .expect("run rustc --print sysroot");
    assert!(sysroot.status.success());
    let sysroot_path = String::from_utf8_lossy(&sysroot.stdout).trim().to_string();
    assert!(!sysroot_path.is_empty());
    assert!(std::path::Path::new(&sysroot_path).is_dir());
}

#[test]
fn detect_uses_regseq_driver_env_var() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let driver = NamedTempFile::new().expect("temporary driver");
    let driver_path = driver.path().to_string_lossy().to_string();
    let _set_driver = EnvVarRestore::set("REGSEQ_DRIVER", &driver_path);

    let info = ToolchainInfo::detect().expect("detection should succeed");
    assert_eq!(info.driver, driver.path());
    assert!(info.rustc.is_file());
    assert!(info.sysroot.is_dir());
}

#[test]
#[ignore = "requires an isolated process environment without rustup on PATH"]
fn detect_errors_when_rustup_unavailable() {
    let error = ToolchainInfo::detect().expect_err("detection should fail");
    assert!(matches!(error, Error::Toolchain(message) if message.contains("rustup")));
}
