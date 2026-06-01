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
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
    }
}

const FMT: &str = "not direct";

#[no_mangle]
pub extern "C" fn caller() {
    regseq::log::intrinsics::log_info(FMT, &[]);
}
