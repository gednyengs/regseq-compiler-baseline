#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                Str(&'a str),
            }

            #[inline(never)]
            #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {
                unimplemented!()
            }
        }

        macro_rules! info {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {
                $crate::regseq::log::intrinsics::log_info($fmt, &[])
            };
        }
        pub(crate) use info;
    }
}

pub fn caller() {
    regseq::log::info!("event\n");
}
