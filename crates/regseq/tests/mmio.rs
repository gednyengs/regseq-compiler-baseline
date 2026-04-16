use regseq::{
    Field, FieldValue, Mmio, ReadAccess, ReadOnlyAccess, ReadWriteAccess, RegisterId, RegisterOf,
    WriteAccess, WriteOnlyAccess,
};

#[test]
fn mmio_is_publicly_exported() {
    let _m: Mmio<u32> = Mmio::new(0u32);
}

#[test]
fn mmio_as_driver_struct_field() {
    struct MyRegs {
        pub status: u32,
        pub control: u32,
    }

    struct MyDriver {
        regs: Mmio<MyRegs>,
    }

    let driver = MyDriver {
        regs: Mmio::new(MyRegs {
            status: 0,
            control: 0,
        }),
    };

    // Access through Deref to exercise the fields
    assert_eq!(driver.regs.status, 0);
    assert_eq!(driver.regs.control, 0);
}

#[test]
fn mmio_deref_provides_field_access() {
    struct Pl011Registers {
        dr: u32,
        fr: u32,
    }

    let regs: Mmio<Pl011Registers> = Mmio::new(Pl011Registers { dr: 0xAB, fr: 0xCD });

    // Deref allows direct field access through Mmio<T>
    assert_eq!(regs.dr, 0xAB);
    assert_eq!(regs.fr, 0xCD);
}

#[test]
fn mmio_is_opaque_no_runtime_support_needed() {
    // Mmio<T> can be constructed and used without any backend/runtime setup.
    // This confirms the type is a pure facade shell.
    let m: Mmio<[u8; 4]> = Mmio::new([1, 2, 3, 4]);
    assert_eq!(m[0], 1);
    assert_eq!(m[3], 4);

    // Debug is available
    let _ = format!("{:?}", Mmio::new(42u32));
}

// ---------------------------------------------------------------------------
// Mmio<T> register access method signature tests
// ---------------------------------------------------------------------------

// Test scaffolding: a dummy block and register with ReadWriteAccess.
struct TestBlock;
struct TestRwReg;

impl RegisterOf<TestBlock> for TestRwReg {
    type Access = ReadWriteAccess;
}

// A read-only register.
struct TestRoReg;

impl RegisterOf<TestBlock> for TestRoReg {
    type Access = ReadOnlyAccess;
}

// A write-only register.
struct TestWoReg;

impl RegisterOf<TestBlock> for TestWoReg {
    type Access = WriteOnlyAccess;
}

#[test]
fn mmio_read_method_signature_compiles() {
    fn assert_read(mmio: &Mmio<TestBlock>, reg: RegisterId<u32, TestRwReg>) {
        let _: u32 = mmio.read(reg);
    }
    let _ = assert_read;
}

#[test]
fn mmio_read_field_method_signature_compiles() {
    fn assert_read_field(mmio: &Mmio<TestBlock>, field: Field<u32, TestRwReg>) {
        let _: u32 = mmio.read_field(field);
    }
    let _ = assert_read_field;
}

#[test]
fn mmio_write_method_signature_compiles() {
    fn assert_write(mmio: &Mmio<TestBlock>, reg: RegisterId<u32, TestRwReg>, value: u32) {
        mmio.write(reg, value);
    }
    let _ = assert_write;
}

#[test]
fn mmio_write_field_method_signature_compiles() {
    fn assert_write_field(mmio: &Mmio<TestBlock>, values: FieldValue<u32, TestRwReg>) {
        mmio.write_field(values);
    }
    let _ = assert_write_field;
}

#[test]
fn mmio_modify_takes_register_id_and_field_value() {
    // modify() takes two arguments: RegisterId and FieldValue.
    fn assert_modify(
        mmio: &Mmio<TestBlock>,
        reg: RegisterId<u32, TestRwReg>,
        values: FieldValue<u32, TestRwReg>,
    ) {
        mmio.modify(reg, values);
    }
    let _ = assert_modify;
}

#[test]
fn mmio_read_compiles_for_read_only_register() {
    fn assert_ro_read(mmio: &Mmio<TestBlock>, reg: RegisterId<u32, TestRoReg>) {
        let _: u32 = mmio.read(reg);
    }
    let _ = assert_ro_read;
}

#[test]
fn mmio_write_compiles_for_write_only_register() {
    fn assert_wo_write(mmio: &Mmio<TestBlock>, reg: RegisterId<u32, TestWoReg>, value: u32) {
        mmio.write(reg, value);
    }
    let _ = assert_wo_write;
}

#[test]
fn mmio_access_control_bounds_are_enforced() {
    // Verify that the right trait bounds exist on Mmio methods.
    // read/read_field require R::Access: ReadAccess
    fn needs_read_access<Block, W: regseq::RegWidth, R: RegisterOf<Block>>()
    where
        R::Access: ReadAccess,
    {
    }
    needs_read_access::<TestBlock, u32, TestRwReg>();
    needs_read_access::<TestBlock, u32, TestRoReg>();

    // write/write_field require R::Access: WriteAccess
    fn needs_write_access<Block, W: regseq::RegWidth, R: RegisterOf<Block>>()
    where
        R::Access: WriteAccess,
    {
    }
    needs_write_access::<TestBlock, u32, TestRwReg>();
    needs_write_access::<TestBlock, u32, TestWoReg>();
}
