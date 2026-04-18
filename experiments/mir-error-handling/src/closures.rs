// MIR Closure Verification
//
// How do closures appear in MIR at mir_drops_elaborated_and_const_checked
// with mir_opt_level = 0?
//
// Patterns to verify:
//   1. Non-capturing closure (|| expr)
//   2. Capturing closure (|x| expr using captured variables)
//   3. Closure passed to a generic function (like poll::until)
//   4. Option::map with a closure

use crate::Error;

// ---------------------------------------------------------------------------
// Simulates poll::until -- generic over a FnMut() -> bool predicate
// ---------------------------------------------------------------------------
#[inline(never)]
pub fn poll_until<C>(mut condition: C, max_iters: u32) -> Result<(), Error>
where
    C: FnMut() -> bool,
{
    for _ in 0..max_iters {
        if condition() {
            return Ok(());
        }
    }
    Err(Error(0))
}

// ---------------------------------------------------------------------------
// Test 1: Non-capturing closure passed to a generic function
//
// Question: How does MIR represent || true? Is it a zero-sized type?
// ---------------------------------------------------------------------------
#[inline(never)]
pub fn read_reg() -> u32 {
    std::hint::black_box(42u32)
}

pub fn test_non_capturing_closure() -> Result<(), Error> {
    poll_until(|| read_reg() & 0x1 != 0, 1000)
}

// ---------------------------------------------------------------------------
// Test 2: Capturing closure
//
// Question: How does MIR represent a closure that captures a local?
// What does the Aggregate::Closure look like?
// ---------------------------------------------------------------------------
pub fn test_capturing_closure(threshold: u32) -> Result<(), Error> {
    poll_until(|| read_reg() > threshold, 1000)
}

// ---------------------------------------------------------------------------
// Test 3: Option::map with a method reference
//
// Question: How does .map(SomeType::method) appear vs .map(|x| x.method())?
// ---------------------------------------------------------------------------
pub struct ClockHandle {
    rate: u32,
}

impl ClockHandle {
    pub fn get_rate(&self) -> u32 {
        self.rate
    }
}

pub fn test_option_map_method_ref(clock: Option<&ClockHandle>) -> u32 {
    clock.map(ClockHandle::get_rate).unwrap_or(48_000_000)
}

pub fn test_option_map_closure(clock: Option<&ClockHandle>) -> u32 {
    clock.map(|c| c.get_rate()).unwrap_or(48_000_000)
}
