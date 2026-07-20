#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// Timed-cpu-delay-and-sleep (TCD construct) host behavioral oracle (T07,
// completing the M1 vertical slice). A process-context routine interleaves an
// observable log trace with delay intrinsics:
//   * a CONST busy_wait (a fixed post-write settle, 10 us);
//   * a RUNTIME-valued busy_wait (the EEPROM `delay_usec` pattern, value behind
//     an `#[inline(never)]` boundary so it lowers through the value-ref path);
//   * a process-context `sleep` (legal — TCD-ERR-01 rejects sleep only in a
//     marked IRQ region).
// The harness fake clock records each `{kind, ns, seq}` event and advances a
// virtual clock; the events interleave with the `[INFO]` log lines by execution
// order, so amount + order + intent are all observable, and intent is preserved
// as hook identity (a busy_wait never prints as a sleep). No real time passes.

// -- regseq::delay (timed-cpu-delay-and-sleep facade) -------------------------
// The opaque nanosecond `Duration` value type plus the two intrinsics. Inlined
// per fixture (identity is the `#[rseq::framework_type]` / `#[rseq::intrinsic]`
// attribute, not a facade crate), mirroring how the diag fixtures inline their
// log surface.
#[rseq::framework_type("duration")]
#[derive(Clone, Copy)]
pub struct Duration {
    ns: u64,
}

impl Duration {
    // Plain multiplication (not the canonical saturating ctors): this fixture
    // is a host-compiled test double for the delay LOWERING, and its tiny
    // values never overflow. The saturating-constructor semantics are proven
    // separately by a compile-time conformance assertion elsewhere (T01).
    pub const fn from_us(us: u64) -> Self {
        Self { ns: us * 1_000 }
    }
    pub const fn from_ms(ms: u64) -> Self {
        Self { ns: ms * 1_000_000 }
    }
}

#[inline(never)]
#[rseq::intrinsic("busy_wait")]
pub fn busy_wait(_d: Duration) {}

#[inline(never)]
#[rseq::intrinsic("sleep")]
pub fn sleep(_d: Duration) {}

// -- regseq::log (the interleaving trace) -------------------------------------
pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                U8(u8), U16(u16), U32(u32), U64(u64),
                I8(i8), I16(i16), I32(i32), I64(i64),
                Bool(bool), Str(&'a str),
            }
            impl From<u32> for LogArg<'_> { fn from(v: u32) -> Self { LogArg::U32(v) } }
            impl<'a> From<&'a str> for LogArg<'a> { fn from(v: &'a str) -> Self { LogArg::Str(v) } }

            #[inline(never)] #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
        macro_rules! info { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_info($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use info;
    }
}

/// A runtime-valued delay amount (microseconds). The `#[inline(never)]`
/// boundary keeps it out of const-folding, so `Duration::from_us(delay_usec())`
/// lowers as a value-ref operand rather than a const aggregate (D04/R04).
#[inline(never)]
fn delay_usec() -> u64 {
    500
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    regseq::log::info!("reset assert");
    // Fixed post-write settle: 10 us = 10_000 ns. Built by an inline
    // `Duration::from_us` call, so the argument is a value-ref place (the
    // constructor result), not a const item — a `const Duration` would fold to
    // a single-field-struct scalar const the bridge does not lower
    // (`UnsupportedConst`), and the corpus delay sites are all inline
    // constructor calls, so this is the faithful operand shape.
    busy_wait(Duration::from_us(10));
    regseq::log::info!("reset deassert");

    // EEPROM-style runtime settle: 500 us = 500_000 ns (value-ref path).
    busy_wait(Duration::from_us(delay_usec()));
    regseq::log::info!("eeprom ready");

    // Process-context scheduler sleep: 5 ms = 5_000_000 ns.
    sleep(Duration::from_ms(5));
    regseq::log::info!("link up");
}
