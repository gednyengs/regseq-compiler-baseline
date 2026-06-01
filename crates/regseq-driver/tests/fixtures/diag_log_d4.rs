#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                F32(f32),
                Str(&'a str),
            }

            impl From<f32> for LogArg<'_> {
                fn from(v: f32) -> Self {
                    LogArg::F32(v)
                }
            }

            #[inline(never)]
            #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }

        macro_rules! info {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {{
                $crate::regseq::log::intrinsics::log_info(
                    $fmt,
                    &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*],
                )
            }};
        }
        pub(crate) use info;
    }
}

#[no_mangle]
pub extern "C" fn caller() {
    regseq::log::info!("float {}", 1.0f32);
}
