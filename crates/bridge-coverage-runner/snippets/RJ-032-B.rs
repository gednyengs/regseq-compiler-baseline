#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                Str(&'a str),
            }

            impl<'a> From<&'a str> for LogArg<'a> {
                fn from(s: &'a str) -> LogArg<'a> {
                    LogArg::Str(s)
                }
            }

            #[inline(never)]
            #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {
                unimplemented!()
            }
        }
    }
}

pub fn caller(s: &str) {
    regseq::log::intrinsics::log_info(
        "x={:x}",
        &[regseq::log::intrinsics::LogArg::from(s)],
    );
}
