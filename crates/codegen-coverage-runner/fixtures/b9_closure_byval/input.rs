#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// B9 regression — a NON-capturing closure passed BY VALUE to a generic fn that
// CALLS it. `apply(|x| x + 2, 40)` exercises:
//   * TY-15 (Closure type)          — `|x| x + 2` has `Ty::Closure`.
//   * AG-6 (AggregateKind::Closure) — constructing the (zero-field) closure.
// The closure captures nothing, so `F` is a ZERO-SIZED type. `apply<F: Fn(..)>`
// takes `f: F` by value (a ZST arg, elided from the C signature), and `f(x)`
// lowers to `<F as Fn>::call(&f, (x,))` — a bare borrow `&f` of that ZST arg.
// The backend formerly declared no storage for a ZST local, so the emitted
// `&(_n)` named an undeclared local and `cc` rejected it. Plan 09 gives a
// bare-borrowed ZST local real zero-sized storage. The single `info!` drives
// the computed result THROUGH the by-value closure call and logs it, so the
// dual oracle proves "compiled + printed the right value".
// `// proves:` maps the line to the Doc-1 matrix row(s) it drives.

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
            #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
        macro_rules! info {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {
                $crate::regseq::log::intrinsics::log_info(
                    $fmt,
                    &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*],
                )
            };
        }
        pub(crate) use info;
    }
}

// Generic over the closure type `F`, taking it BY VALUE. `f(x)` borrows the
// (zero-sized) `f` to reach the `Fn::call` shim — the bare ZST borrow under test.
#[inline(never)]
fn apply<F: Fn(u32) -> u32>(f: F, x: u32) -> u32 {
    f(x)
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    // TY-15 Closure + AG-6 AggregateKind::Closure: a non-capturing (ZST) closure
    // passed by value into `apply`, which calls it through `&f`.
    let r = apply(|x| x + 2, 40);
    regseq::log::info!("closure_byval={}", r); // 42  proves: TY-15 (Closure), AG-6 (AggregateKind::Closure)
}
