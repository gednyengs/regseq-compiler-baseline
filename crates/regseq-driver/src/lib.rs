#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_abi;
extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

pub mod args;
pub mod bridge;
mod callbacks;
pub use callbacks::{regseq_mir_passes, RegseqAwareCallbacks, RegseqCallbacks};

const BUG_REPORT_URL: &str = "https://github.com/gednyengs/regseq-compiler-asplos/issues";

/// Run the regseq-driver compiler wrapper.
///
/// Initializes the rustc logger, installs an ICE hook for friendly panic
/// reporting, then invokes the compiler. Returns an exit code (`i32`)
/// suitable for passing to `std::process::exit()`.
pub fn run() -> i32 {
    let early_dcx =
        rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
    rustc_driver::init_rustc_env_logger(&early_dcx);
    rustc_driver::install_ice_hook(BUG_REPORT_URL, |_| {});

    rustc_driver::catch_with_exit_code(|| {
        let args = args::DriverArgs::parse_from(&std::env::args().collect::<Vec<_>>());

        let mut rustc_args = Vec::new();
        if let Some(ref path) = args.rustc_path {
            rustc_args.push(path.clone());
        }
        rustc_args.extend(args.rustc_args.iter().cloned());

        if let Ok(sysroot) = std::env::var("REGSEQ_SYSROOT") {
            if !args::has_arg(&rustc_args, "--sysroot") {
                rustc_args.push("--sysroot".to_string());
                rustc_args.push(sysroot);
            }
        }

        match args.dispatch() {
            args::DriverMode::Passthrough => {
                rustc_driver::run_compiler(&rustc_args, &mut callbacks::PassthroughCallbacks);
            }
            args::DriverMode::RegseqAware => {
                rustc_driver::run_compiler(&rustc_args, &mut callbacks::RegseqAwareCallbacks);
            }
            args::DriverMode::Regseq => {
                let options_json = std::env::var("REGSEQ_OPTIONS")
                    .expect("REGSEQ_OPTIONS must be set in regseq mode");
                let options: regseq_config::CompilerOptions = serde_json::from_str(&options_json)
                    .unwrap_or_else(|error| {
                        panic!(
                            "REGSEQ_OPTIONS contains invalid JSON: {error}\nraw: {options_json}"
                        );
                    });
                rustc_driver::run_compiler(
                    &rustc_args,
                    &mut callbacks::RegseqCallbacks::new(options),
                );
            }
        }
    })
}
