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

        macro_rules! info_one {
            ($fmt:literal, $a:expr $(,)?) => {
                $crate::regseq::log::intrinsics::log_info(
                    $fmt,
                    &[$crate::regseq::log::intrinsics::LogArg::Str($a)],
                )
            };
        }
        pub(crate) use info_one;
    }
}

pub fn caller() {
    regseq::log::info_one!("x={}{}", "foo");
}
