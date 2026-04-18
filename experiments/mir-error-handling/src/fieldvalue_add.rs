// MIR FieldValue Addition Verification
//
// Question: At mir_opt_level=0, does FieldValue<T,R> addition via the
// `+` operator appear as a Call to <FieldValue as Add>::add, or is it
// const-evaluated / inlined?

use core::marker::PhantomData;
use core::ops::Add;

#[derive(Clone, Copy)]
pub struct FieldValue<T, R> {
    pub mask: T,
    pub value: T,
    _reg: PhantomData<R>,
}

impl<T, R> FieldValue<T, R> {
    pub const fn new(mask: T, value: T) -> Self {
        Self { mask, value, _reg: PhantomData }
    }
}

impl<R> Add for FieldValue<u32, R> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            mask: self.mask | rhs.mask,
            value: self.value | rhs.value,
            _reg: PhantomData,
        }
    }
}

// Simulated register descriptor
pub mod CR_REG {
    use super::*;
    pub enum Register {}

    pub mod UARTEN {
        use super::*;
        pub const ENABLED: FieldValue<u32, super::Register> = FieldValue::new(0x1, 0x1);
        pub const DISABLED: FieldValue<u32, super::Register> = FieldValue::new(0x1, 0x0);
    }
    pub mod TXE {
        use super::*;
        pub const ENABLED: FieldValue<u32, super::Register> = FieldValue::new(0x100, 0x100);
        pub const DISABLED: FieldValue<u32, super::Register> = FieldValue::new(0x100, 0x0);
    }
    pub mod RXE {
        use super::*;
        pub const ENABLED: FieldValue<u32, super::Register> = FieldValue::new(0x200, 0x200);
        pub const DISABLED: FieldValue<u32, super::Register> = FieldValue::new(0x200, 0x0);
    }
}

/// Test: composing three field values with `+`
/// This is the exact PL011 pattern:
///   CR_REG::UARTEN::ENABLED + CR_REG::TXE::ENABLED + CR_REG::RXE::ENABLED
pub fn test_fieldvalue_add() -> FieldValue<u32, CR_REG::Register> {
    CR_REG::UARTEN::ENABLED + CR_REG::TXE::ENABLED + CR_REG::RXE::ENABLED
}

/// Test: composing in a function that uses the result
pub fn test_fieldvalue_add_and_use() -> (u32, u32) {
    let combined = CR_REG::UARTEN::DISABLED + CR_REG::TXE::DISABLED + CR_REG::RXE::DISABLED;
    (combined.mask, combined.value)
}
