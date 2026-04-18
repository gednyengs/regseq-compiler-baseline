pub mod closures;
pub mod fieldvalue_add;
pub mod register_access;

// MIR Error Handling Verification
//
// Compiled with: rustc +nightly --emit=mir -Z mir-opt-level=0
//
// We inspect the MIR at `mir_drops_elaborated_and_const_checked` stage.
// The --emit=mir flag dumps MIR after drop elaboration by default.
//
// Verification goals:
//   1. How fallible operations (returning Result) are represented in MIR
//   2. How Drop is elaborated on error paths
//   3. Whether error paths use SwitchInt + Drop, or Call's unwind path

// ---------------------------------------------------------------------------
// Setup: types with Drop, fallible functions
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Error(pub u32);

/// A guard type that implements Drop (simulates LockGuard, resource handle, etc.)
pub struct Guard {
    id: u32,
}

impl Drop for Guard {
    fn drop(&mut self) {
        // In a real driver, this would call an unlock/cleanup intrinsic.
        // Using black_box to prevent optimization removal.
        std::hint::black_box(self.id);
    }
}

/// Simulates a fallible resource acquisition (like lock(), enable(), etc.)
#[inline(never)]
pub fn acquire(id: u32) -> Result<Guard, Error> {
    if id > 0 {
        Ok(Guard { id })
    } else {
        Err(Error(id))
    }
}

/// Simulates a fallible operation that doesn't return a resource.
#[inline(never)]
pub fn fallible_op(val: u32) -> Result<u32, Error> {
    if val > 10 {
        Ok(val)
    } else {
        Err(Error(val))
    }
}

// ---------------------------------------------------------------------------
// Test 1: Single ? with one Droppable local
//
// Question: When fallible_op fails, does the error path contain Drop(guard)?
// ---------------------------------------------------------------------------
pub fn test_single_guard() -> Result<u32, Error> {
    let guard = acquire(1)?;
    let result = fallible_op(guard.id)?;
    drop(guard);
    Ok(result)
}

// ---------------------------------------------------------------------------
// Test 2: Cascading ? with multiple Droppable locals
//
// Question: When the third acquire fails, do we see Drop for b then a?
// ---------------------------------------------------------------------------
pub fn test_cascading_guards() -> Result<u32, Error> {
    let a = acquire(1)?;
    let b = acquire(2)?;
    let c = acquire(3)?;
    let sum = a.id + b.id + c.id;
    drop(c);
    drop(b);
    drop(a);
    Ok(sum)
}

// ---------------------------------------------------------------------------
// Test 3: ? inside a scope with a guard (simulates lock guard pattern)
//
// Question: Does the error path from fallible_op drop the guard?
// ---------------------------------------------------------------------------
pub fn test_guard_with_fallible_inner() -> Result<u32, Error> {
    let guard = acquire(1)?;
    let val = fallible_op(42)?;
    let result = val + guard.id;
    drop(guard);
    Ok(result)
}

// ---------------------------------------------------------------------------
// Test 4: Conditional guard acquisition
//
// Question: How does MIR handle dropping a conditionally-acquired guard?
// ---------------------------------------------------------------------------
pub fn test_conditional_guard(condition: bool) -> Result<u32, Error> {
    let a = acquire(1)?;
    let b = if condition {
        Some(acquire(2)?)
    } else {
        None
    };
    let val = fallible_op(100)?;
    let b_val = b.as_ref().map_or(0, |g| g.id);
    let result = a.id + b_val + val;
    drop(b);
    drop(a);
    Ok(result)
}

// ---------------------------------------------------------------------------
// Test 5: User-defined type with Drop containing a method call
//
// Question: Is the Drop impl body visible? Does it show the method call?
// ---------------------------------------------------------------------------
pub struct ResourceHandle {
    active: bool,
}

impl ResourceHandle {
    #[inline(never)]
    pub fn cleanup(&mut self) {
        self.active = false;
        std::hint::black_box(self.active);
    }
}

impl Drop for ResourceHandle {
    fn drop(&mut self) {
        if self.active {
            self.cleanup();
        }
    }
}

#[inline(never)]
pub fn acquire_resource() -> Result<ResourceHandle, Error> {
    Ok(ResourceHandle { active: true })
}

pub fn test_drop_with_method_call() -> Result<u32, Error> {
    let handle = acquire_resource()?;
    let val = fallible_op(42)?;
    let result = if handle.active { val } else { 0 };
    drop(handle);
    Ok(result)
}
