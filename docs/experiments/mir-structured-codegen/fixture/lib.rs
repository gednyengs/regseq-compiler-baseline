// ---------------------------------------------------------------------------
// Codegen experiment fixture
//
// Exercises three patterns relevant to device driver C code generation:
//   A) Sequential resource acquisition with error-checked early returns (probe)
//   B) Multi-arm match dispatch (interrupt handler)
//   C) Loop with conditional break (register polling)
//
// The types and intrinsics here are minimal facades -- just enough to produce
// meaningful MIR with Drop elaboration and intrinsic call sites.
// ---------------------------------------------------------------------------

// -- Intrinsics --------------------------------------------------------------

/// Allocate `count` elements of type `T`.
///
/// The regseq compiler recognises this call site and replaces it with an
/// OS-specific allocation (e.g. `kmalloc` on Linux, `k_malloc` on Zephyr).
#[rseq::intrinsic]
#[inline(never)]
pub unsafe fn alloc<T>(count: usize) -> *mut T {
    // Body is never executed -- the compiler intercepts the call.
    let _ = count;
    core::ptr::null_mut()
}

/// Free memory previously obtained from [`alloc`].
#[rseq::intrinsic]
#[inline(never)]
pub unsafe fn free<T>(ptr: *mut T) {
    let _ = ptr;
}

// -- Facade type: RegseqVec -------------------------------------------------

/// Minimal growable-buffer facade that exercises `Drop` elaboration.
///
/// NOT a real Vec -- only enough surface area to produce the MIR patterns
/// we want to study (allocation, null-check, drop with cleanup).
pub struct RegseqVec<T> {
    ptr: *mut T,
    len: usize,
    cap: usize,
}

impl<T> RegseqVec<T> {
    /// Allocate a buffer with the given capacity via the `alloc` intrinsic.
    pub fn with_capacity(cap: usize) -> Self {
        let ptr = unsafe { alloc::<T>(cap) };
        Self { ptr, len: 0, cap }
    }

    /// Returns `true` when the backing allocation is null (allocation failed).
    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    /// Current number of initialised elements.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the buffer contains no initialised elements.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Allocated capacity in number of elements.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

impl<T> Drop for RegseqVec<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { free(self.ptr) }
        }
    }
}

// -- Driver struct -----------------------------------------------------------

#[rseq::driver(name = "experiment")]
pub struct ExpDriver {
    pub base_addr: usize,
}

// -- Pattern A: sequential acquire / check / cleanup -------------------------

/// Probe-like function: acquire two buffers in sequence, checking each for
/// failure. On error the already-acquired resources must be cleaned up.
///
/// The MIR will show `Drop` terminators inserted by drop elaboration at each
/// early-return point and at the normal exit.
#[rseq::probe]
pub fn probe_like(addr: usize) -> Result<ExpDriver, i32> {
    let buf_a: RegseqVec<u8> = RegseqVec::with_capacity(64);
    if buf_a.is_null() {
        return Err(-1);
    }

    let buf_b: RegseqVec<u8> = RegseqVec::with_capacity(128);
    if buf_b.is_null() {
        return Err(-2);
    }

    let _total = buf_a.len() + buf_b.len();
    Ok(ExpDriver { base_addr: addr })
}

// -- Pattern B: multi-arm match (switch dispatch) ----------------------------

/// Interrupt-handler-like function: dispatch on a status word.
///
/// The MIR will contain a `SwitchInt` terminator with multiple targets.
#[rseq::interrupt]
pub fn handle_status(status: u32) -> u32 {
    match status {
        0 => 0,
        1 => status + 10,
        2..=5 => status * 2,
        _ => 0xFF,
    }
}

// -- Pattern C: loop with conditional break ----------------------------------

/// Poll a (simulated) register up to `max_retries` times.
///
/// The MIR will contain a natural loop (back-edge to the loop header) and a
/// conditional break (`return true`) inside the loop body.
pub fn poll_register(max_retries: u32) -> bool {
    let mut i: u32 = 0;
    while i < max_retries {
        if i == 42 {
            return true;
        }
        i += 1;
    }
    false
}
