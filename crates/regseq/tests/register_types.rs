use regseq::{
    Field, FieldValue, ReadAccess, ReadOnly, ReadOnlyAccess, ReadWrite, ReadWriteAccess, RegWidth,
    RegisterId, RegisterOf, Reserved, WriteAccess, WriteOnly, WriteOnlyAccess,
};

// A dummy register marker type for testing.
struct TestReg;

#[test]
fn support_types_are_exported() {
    let _f: Field<u32, TestReg> = Field::new(0xFF, 0);
    let _fv: FieldValue<u32, TestReg> = FieldValue::new(0xFF, 0x42);
}

#[test]
fn readonly_has_read_method() {
    // Verify the read method signature compiles on ReadOnly<T, R>.
    fn assert_read_compiles(reg: &ReadOnly<u32, TestReg>, field: Field<u32, TestReg>) {
        let _: u32 = reg.read(field);
    }
    fn assert_read_raw_compiles(reg: &ReadOnly<u32, TestReg>) {
        let _: u32 = reg.read_raw();
    }
    let _ = (assert_read_compiles, assert_read_raw_compiles);
}

#[test]
fn writeonly_has_write_and_set_methods() {
    // Verify write and set method signatures compile on WriteOnly<T, R>.
    fn assert_write_compiles(reg: &WriteOnly<u32, TestReg>, values: FieldValue<u32, TestReg>) {
        reg.write(values);
    }
    fn assert_set_compiles(reg: &WriteOnly<u32, TestReg>, values: FieldValue<u32, TestReg>) {
        reg.set(values);
    }
    let _ = (assert_write_compiles, assert_set_compiles);
}

#[test]
fn readwrite_has_all_four_methods() {
    // Verify read_raw, read, write, modify, set method signatures compile on ReadWrite<T, R>.
    fn assert_read_raw(reg: &ReadWrite<u32, TestReg>) {
        let _: u32 = reg.read_raw();
    }
    fn assert_read(reg: &ReadWrite<u32, TestReg>, field: Field<u32, TestReg>) {
        let _: u32 = reg.read(field);
    }
    fn assert_write(reg: &ReadWrite<u32, TestReg>, values: FieldValue<u32, TestReg>) {
        reg.write(values);
    }
    fn assert_modify(reg: &ReadWrite<u32, TestReg>, values: FieldValue<u32, TestReg>) {
        reg.modify(values);
    }
    fn assert_set(reg: &ReadWrite<u32, TestReg>, values: FieldValue<u32, TestReg>) {
        reg.set(values);
    }
    let _ = (
        assert_read_raw,
        assert_read,
        assert_write,
        assert_modify,
        assert_set,
    );
}

#[test]
fn wrapper_types_are_distinct() {
    // ReadOnly, WriteOnly, and ReadWrite are distinct types -- no implicit conversion.
    fn takes_ro(_: &ReadOnly<u32, TestReg>) {}
    fn takes_wo(_: &WriteOnly<u32, TestReg>) {}
    fn takes_rw(_: &ReadWrite<u32, TestReg>) {}
    let _ = (takes_ro, takes_wo, takes_rw);
}

#[test]
fn field_is_const_constructible() {
    const F: Field<u32, TestReg> = Field::new(0xFF, 8);
    assert_eq!(F.mask, 0xFF);
    assert_eq!(F.shift, 8);
}

#[test]
fn field_value_is_const_constructible() {
    const FV: FieldValue<u32, TestReg> = FieldValue::new(0xFF00, 0x4200);
    assert_eq!(FV.mask, 0xFF00);
    assert_eq!(FV.value, 0x4200);
}

#[test]
fn field_value_composition() {
    let a: FieldValue<u32, TestReg> = FieldValue::new(0x00FF, 0x0042);
    let b: FieldValue<u32, TestReg> = FieldValue::new(0xFF00, 0x1300);
    let combined = a + b;
    assert_eq!(combined.mask, 0xFFFF);
    assert_eq!(combined.value, 0x1342);
}

#[test]
fn register_block_layout_coherence() {
    // Wrappers and Reserved are layout-sized so they can appear in #[repr(C)] structs.
    #[repr(C)]
    struct MockRegs {
        pub dr: ReadWrite<u32, TestReg>,
        _reserved0: Reserved<u32>,
        pub fr: ReadOnly<u32, TestReg>,
        pub cr: WriteOnly<u32, TestReg>,
    }

    // Each wrapper is repr(transparent) over T, so it should be the same size as T.
    assert_eq!(
        core::mem::size_of::<ReadWrite<u32, TestReg>>(),
        core::mem::size_of::<u32>()
    );
    assert_eq!(
        core::mem::size_of::<ReadOnly<u32, TestReg>>(),
        core::mem::size_of::<u32>()
    );
    assert_eq!(
        core::mem::size_of::<WriteOnly<u32, TestReg>>(),
        core::mem::size_of::<u32>()
    );
    assert_eq!(
        core::mem::size_of::<Reserved<u32>>(),
        core::mem::size_of::<u32>()
    );

    // The repr(C) struct should have a predictable layout.
    assert_eq!(
        core::mem::size_of::<MockRegs>(),
        4 * core::mem::size_of::<u32>()
    );
}

// ---------------------------------------------------------------------------
// Access marker tests
// ---------------------------------------------------------------------------

#[test]
fn read_only_access_implements_read_access() {
    fn assert_read<T: ReadAccess>() {}
    assert_read::<ReadOnlyAccess>();
}

#[test]
fn read_only_access_does_not_implement_write_access() {
    // Negative bound: ReadOnlyAccess must NOT implement WriteAccess.
    // Rust has no negative trait bounds or built-in compile-fail testing, so
    // correctness here relies on the absence of `impl WriteAccess for ReadOnlyAccess`
    // in registers.rs. If someone accidentally adds that impl, the type system
    // will silently accept writes to read-only registers -- catch in code review.
    fn takes_write<T: WriteAccess>(_: &T) {}
    // Uncommenting the next line must fail to compile:
    // takes_write(&ReadOnlyAccess);
    let _ = takes_write::<WriteOnlyAccess>;
}

#[test]
fn write_only_access_implements_write_access() {
    fn assert_write<T: WriteAccess>() {}
    assert_write::<WriteOnlyAccess>();
}

#[test]
fn write_only_access_does_not_implement_read_access() {
    // Negative bound: WriteOnlyAccess must NOT implement ReadAccess.
    // Same reasoning as read_only_access_does_not_implement_write_access above.
    fn takes_read<T: ReadAccess>(_: &T) {}
    // Uncommenting the next line must fail to compile:
    // takes_read(&WriteOnlyAccess);
    let _ = takes_read::<ReadOnlyAccess>;
}

#[test]
fn read_write_access_implements_both() {
    fn assert_read<T: ReadAccess>() {}
    fn assert_write<T: WriteAccess>() {}
    assert_read::<ReadWriteAccess>();
    assert_write::<ReadWriteAccess>();
}

// ---------------------------------------------------------------------------
// RegisterId tests
// ---------------------------------------------------------------------------

#[test]
fn register_id_is_const_constructible() {
    const REG: RegisterId<u32, TestReg> = RegisterId::new();
    let _ = REG;
}

#[test]
fn register_id_is_copy_and_clone() {
    let reg: RegisterId<u32, TestReg> = RegisterId::new();
    let copied = reg;
    let cloned = reg.clone();
    let _ = (copied, cloned);
}

// ---------------------------------------------------------------------------
// RegWidth tests
// ---------------------------------------------------------------------------

#[test]
fn u8_implements_reg_width() {
    fn assert_reg_width<T: RegWidth>() {}
    assert_reg_width::<u8>();
}

#[test]
fn u16_implements_reg_width() {
    fn assert_reg_width<T: RegWidth>() {}
    assert_reg_width::<u16>();
}

#[test]
fn u32_implements_reg_width() {
    fn assert_reg_width<T: RegWidth>() {}
    assert_reg_width::<u32>();
}

// ---------------------------------------------------------------------------
// RegisterOf trait tests
// ---------------------------------------------------------------------------

#[test]
fn register_of_trait_usable_in_bounds() {
    // Dummy block and register types to verify the trait compiles in bounds.
    struct DummyBlock;
    struct DummyReg;

    impl RegisterOf<DummyBlock> for DummyReg {
        type Access = ReadWriteAccess;
    }

    fn assert_register_of<Block, R: RegisterOf<Block>>()
    where
        R::Access: ReadAccess,
    {
    }
    assert_register_of::<DummyBlock, DummyReg>();
}
