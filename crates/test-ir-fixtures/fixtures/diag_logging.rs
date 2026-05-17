// One call to each of the four severity macros with a single `u32`
// argument. Consumed by the `regseq-model-94` (P1a-E2E-02) end-to-end
// test via `bridge_fixture(fixture_names::DIAG_LOGGING)`.
//
// The workspace `regseq` facade (`crates/regseq/src/lib.rs`) is a
// deliberate placeholder per the redesign's incremental-promotion
// rule, and the `test-ir-fixtures` helper compiles each fixture as a
// standalone `--crate-type rlib` without `--extern regseq=...`
// (`crates/test-ir-fixtures/src/lib.rs` `compile_and_bridge`). The
// fixture therefore inlines the canonical `regseq::log` shim --
// closed-vocabulary `log_<sev>` intrinsics (per spec.md section 26.3
// + the `#[rseq::intrinsic("log-<sev>")]` recognition tags landed by
// `regseq-driver-44`), the `LogArg` carrier ADT with the by-value
// `From<u32>` / `From<&'a str>` impls per spec.md section 26.4, and
// the four declarative `macro_rules!` severity macros whose
// expansion shape mirrors spec.md section 26.5
// (`&[ LogArg::from($arg) ]` carrier per author argument).
//
// This is the same inline-shim pattern used by
// `crates/regseq-driver/tests/bridge_diag_log_recognition.rs`; the
// only material change is that the macros here expand to the full
// carrier shape (not the empty `&[]` shortcut the recognition tests
// use) so the DiagLogPeel pass exercises its slice-aggregate
// traversal path end-to-end.
#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                U32(u32),
                Str(&'a str),
            }

            impl<'a> From<u32> for LogArg<'a> {
                fn from(v: u32) -> Self {
                    LogArg::U32(v)
                }
            }

            impl<'a> From<&'a str> for LogArg<'a> {
                fn from(s: &'a str) -> Self {
                    LogArg::Str(s)
                }
            }

            #[inline(never)]
            #[rseq::intrinsic("log-debug")]
            pub fn log_debug(_fmt: &'static str, _args: &[LogArg<'_>]) {
                unimplemented!()
            }

            #[inline(never)]
            #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {
                unimplemented!()
            }

            #[inline(never)]
            #[rseq::intrinsic("log-warn")]
            pub fn log_warn(_fmt: &'static str, _args: &[LogArg<'_>]) {
                unimplemented!()
            }

            #[inline(never)]
            #[rseq::intrinsic("log-error")]
            pub fn log_error(_fmt: &'static str, _args: &[LogArg<'_>]) {
                unimplemented!()
            }
        }

        macro_rules! debug {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {
                $crate::regseq::log::intrinsics::log_debug(
                    $fmt,
                    &[ $( $crate::regseq::log::intrinsics::LogArg::from($arg) ),* ],
                )
            };
        }
        pub(crate) use debug;

        macro_rules! info {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {
                $crate::regseq::log::intrinsics::log_info(
                    $fmt,
                    &[ $( $crate::regseq::log::intrinsics::LogArg::from($arg) ),* ],
                )
            };
        }
        pub(crate) use info;

        // `warn` collides with the builtin `warn` attribute when
        // declared at this scope, so the macro is authored as
        // `log_warn` and re-exported under the `warn` name (mirroring
        // `crates/regseq-driver/tests/bridge_diag_log_recognition.rs`).
        macro_rules! log_warn {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {
                $crate::regseq::log::intrinsics::log_warn(
                    $fmt,
                    &[ $( $crate::regseq::log::intrinsics::LogArg::from($arg) ),* ],
                )
            };
        }
        pub(crate) use log_warn as warn;

        macro_rules! error {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {
                $crate::regseq::log::intrinsics::log_error(
                    $fmt,
                    &[ $( $crate::regseq::log::intrinsics::LogArg::from($arg) ),* ],
                )
            };
        }
        pub(crate) use error;
    }
}

pub fn emit_logs(retries: u32) {
    regseq::log::debug!("debug event x={}", retries);
    regseq::log::info!("info event x={}", retries);
    regseq::log::warn!("warn event x={}", retries);
    regseq::log::error!("error event x={}", retries);
}
