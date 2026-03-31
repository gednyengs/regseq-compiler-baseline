use regseq::{Completion, Duration, Guard, IrqSpinLock, Mutex, RegseqError, SpinLock};

#[test]
fn sync_types_publicly_available() {
    fn _accepts_mutex<T>(_m: Mutex<T>) {}
    fn _accepts_spinlock<T>(_s: SpinLock<T>) {}
    fn _accepts_irq_spinlock<T>(_i: IrqSpinLock<T>) {}
    fn _accepts_completion(_c: Completion) {}
    fn _accepts_guard<'a, T>(_g: Guard<'a, T>) {}
}

#[test]
fn lock_signatures_compile() {
    // Verify that .lock() returns Guard for all lock types.
    fn _mutex_lock(m: &Mutex<u32>) -> Guard<'_, u32> {
        m.lock()
    }
    fn _spinlock_lock(s: &SpinLock<u32>) -> Guard<'_, u32> {
        s.lock()
    }
    fn _irq_spinlock_lock(i: &IrqSpinLock<u32>) -> Guard<'_, u32> {
        i.lock()
    }
}

#[test]
fn completion_signatures_compile() {
    // Verify wait/signal/reset signatures use the accepted baselines.
    fn _wait(c: &Completion) -> Result<(), RegseqError> {
        c.wait(Duration::from_ms(100))
    }
    fn _signal(c: &Completion) {
        c.signal()
    }
    fn _reset(c: &Completion) {
        c.reset()
    }
}

#[test]
fn guard_supports_deref() {
    // Verify Guard<'_, T> implements Deref<Target = T> and DerefMut.
    fn _read_guard<'a>(g: &'a Guard<'a, u32>) -> &'a u32 {
        &*g
    }
    fn _write_guard<'a>(g: &'a mut Guard<'a, u32>) -> &'a mut u32 {
        &mut *g
    }
}
