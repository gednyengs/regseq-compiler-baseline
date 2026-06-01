pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                U32(u32),
                Str(&'a str),
            }

            impl From<u32> for LogArg<'_> {
                fn from(v: u32) -> Self {
                    LogArg::U32(v)
                }
            }

            impl<'a> From<&'a str> for LogArg<'a> {
                fn from(v: &'a str) -> Self {
                    LogArg::Str(v)
                }
            }

            #[inline(never)]
            #[rseq::intrinsic("log-debug")]
            pub fn log_debug(_fmt: &'static str, _args: &[LogArg<'_>]) {}

            #[inline(never)]
            #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}

            #[inline(never)]
            #[rseq::intrinsic("log-warn")]
            pub fn log_warn(_fmt: &'static str, _args: &[LogArg<'_>]) {}

            #[inline(never)]
            #[rseq::intrinsic("log-error")]
            pub fn log_error(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }

        macro_rules! debug {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {{
                $crate::regseq::log::intrinsics::log_debug(
                    $fmt,
                    &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*],
                )
            }};
        }
        pub(crate) use debug;

        macro_rules! info {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {{
                $crate::regseq::log::intrinsics::log_info(
                    $fmt,
                    &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*],
                )
            }};
        }
        pub(crate) use info;

        macro_rules! log_warn {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {{
                $crate::regseq::log::intrinsics::log_warn(
                    $fmt,
                    &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*],
                )
            }};
        }
        pub(crate) use log_warn as warn;

        macro_rules! error {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {{
                $crate::regseq::log::intrinsics::log_error(
                    $fmt,
                    &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*],
                )
            }};
        }
        pub(crate) use error;
    }
}
