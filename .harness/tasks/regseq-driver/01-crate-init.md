# Task: regseq-driver-01 - Crate initialization

## Description

Initialize the `regseq-driver` crate. This is the rustc wrapper binary that
cargo-regseq invokes via `RUSTC_WRAPPER`. It uses `#![feature(rustc_private)]`
to link against rustc internals.

The binary must be minimal -- `main()` delegates to a library function that
handles ICE hooks, logger initialization, and the compiler invocation. The
library function uses `rustc_driver::catch_with_exit_code` to catch fatal
errors and convert them to exit codes.

## Input Context

Read before starting:
- crates/regseq-driver/README.md
- Cargo.toml (workspace root)
- rust-toolchain.toml

## Output Specification

Create:
- crates/regseq-driver/Cargo.toml
- crates/regseq-driver/build.rs
- crates/regseq-driver/src/main.rs
- crates/regseq-driver/src/lib.rs

Modify:
- Cargo.toml (add `"crates/regseq-driver"` to workspace members)

## Implementation Notes

### Cargo.toml

```toml
[package]
name = "regseq-driver"
version = "0.1.0"
edition = "2021"

[package.metadata.rust-analyzer]
rustc_private = true

[dependencies]
regseq-config = { path = "../regseq-config" }
```

The `[package.metadata.rust-analyzer]` section tells rust-analyzer to resolve
`extern crate rustc_*` declarations against the sysroot. Without it, IDE
support for rustc internals will not work.

No `rustc_driver` or other rustc crates appear in `[dependencies]` -- they
are resolved from the sysroot via `#![feature(rustc_private)]` and
`extern crate` declarations.

### build.rs

Embed the sysroot library path as an rpath so the binary can find
`librustc_driver` and other rustc shared libraries at runtime without
needing `LD_LIBRARY_PATH` or `DYLD_LIBRARY_PATH`:

```rust
fn main() {
    let output = std::process::Command::new(
        std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()),
    )
    .arg("--print=sysroot")
    .output()
    .expect("failed to run rustc --print=sysroot");
    let sysroot = String::from_utf8(output.stdout)
        .expect("sysroot is not utf-8")
        .trim()
        .to_owned();
    println!("cargo:rustc-link-arg=-Wl,-rpath,{sysroot}/lib");
}
```

### main.rs

Minimal binary entry point. Declares `#![feature(rustc_private)]` and
`extern crate rustc_driver` (needed to pull in object code -- other rustc
crates are shipped as rmeta only). Delegates entirely to the library:

```rust
#![feature(rustc_private)]

extern crate rustc_driver;

fn main() {
    std::process::exit(regseq_driver::run())
}
```

This is the entire file. All driver logic lives in `lib.rs`.

Note: `run()` returns `i32` because `rustc_driver::catch_with_exit_code`
returns `i32`. We use `std::process::exit()` to propagate the exit code,
which is the same pattern used by `rustc` itself.

### lib.rs

The library entry point. Declares `extern crate` for all rustc crates
needed by the driver (start with the minimum set; later tasks add more):

```rust
#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
```

Define a bug report URL constant:

```rust
const BUG_REPORT_URL: &str =
    "https://github.com/AeneasVerif/regseq-compiler-v2/issues";
```

Implement the public entry point:

```rust
/// Run the regseq-driver compiler wrapper.
///
/// Initializes the rustc logger, installs an ICE hook for friendly panic
/// reporting, then invokes the compiler. Returns an exit code (`i32`)
/// suitable for passing to `std::process::exit()`.
pub fn run() -> i32 {
    let early_dcx = rustc_session::EarlyDiagCtxt::new(
        rustc_session::config::ErrorOutputType::default(),
    );
    rustc_driver::init_rustc_env_logger(&early_dcx);
    rustc_driver::install_ice_hook(BUG_REPORT_URL, |_| {});

    rustc_driver::catch_with_exit_code(|| {
        let args: Vec<String> = std::env::args().collect();
        rustc_driver::run_compiler(&args, &mut DefaultCallbacks);
    })
}

struct DefaultCallbacks;

impl rustc_driver::Callbacks for DefaultCallbacks {}
```

For this initial task, `run()` simply forwards all arguments to rustc with
empty callbacks. This makes regseq-driver act as a transparent rustc
passthrough. Subsequent tasks add dispatch logic and the regseq pipeline.

The `DefaultCallbacks` struct implements `rustc_driver::Callbacks` with all
default (no-op) methods.

Note: `rustc_driver::catch_with_exit_code` catches fatal errors and panics
from within the compiler and converts them to an `i32` exit code. This is
the same pattern used by `rustc` itself (see `rustc_driver_impl::main()`).

## Test Plan

Create:
- crates/regseq-driver/tests/smoke.rs

The smoke test verifies that the library crate compiles and is importable.
It cannot easily test `run()` since that requires a full rustc invocation
context.

Tests to implement:
1. Smoke test: verify the crate compiles by importing `regseq_driver::run`
   and checking it exists (reference the function without calling it).

Test command: `cargo test -p regseq-driver`

## Acceptance Criteria

- Crate compiles with the pinned nightly toolchain
- Smoke test passes
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean (note: clippy may not lint rustc_private
  code; this is expected)
- `cargo build -p regseq-driver` produces a `regseq-driver` binary
