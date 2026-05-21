#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                Str(&'a str),
                Char(char),
            }

            impl<'a> From<char> for LogArg<'a> {
                fn from(c: char) -> LogArg<'a> {
                    LogArg::Char(c)
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

pub fn caller() {
    regseq::log::intrinsics::log_info(
        "x={}",
        &[regseq::log::intrinsics::LogArg::from('a')],
    );
}
