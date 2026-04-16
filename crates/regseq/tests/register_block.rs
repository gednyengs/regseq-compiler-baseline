#![feature(register_tool)]
#![register_tool(rseq)]

use regseq::{Field, FieldValue, Mmio, ReadAccess, RegisterId, RegisterOf, WriteAccess};

// ---------------------------------------------------------------------------
// Test block definition
// ---------------------------------------------------------------------------

regseq::register_block! {
    #[endian = "little", access_width = 32]
    pub test_block {
        status: ReadOnly {
            BUSY OFFSET(0) NUMBITS(1) [],
            ERROR OFFSET(1) NUMBITS(1) [],
            MODE OFFSET(2) NUMBITS(2) [IDLE = 0, ACTIVE = 1, SLEEP = 2],
        },
        control: ReadWrite {
            ENABLE OFFSET(0) NUMBITS(1) [DISABLED = 0, ENABLED = 1],
            SPEED OFFSET(1) NUMBITS(2) [SLOW = 0, FAST = 1, TURBO = 3],
            RESET OFFSET(3) NUMBITS(1) [],
        },
        _reserved0: Reserved<u32> [2],
        data: ReadWrite {
            VALUE OFFSET(0) NUMBITS(16) [],
            TAG OFFSET(16) NUMBITS(8) [],
        },
    }
}

// ---------------------------------------------------------------------------
// Module structure tests
// ---------------------------------------------------------------------------

#[test]
fn block_module_exists_with_regs_struct() {
    let _regs: test_block::Regs;
    // Regs is a pub struct inside the block module.
}

#[test]
fn per_register_modules_exist() {
    let _: test_block::status::Register;
    let _: test_block::control::Register;
    let _: test_block::data::Register;
}

#[test]
fn register_id_constants_exist() {
    let _: RegisterId<u32, test_block::status::Register> = test_block::status::REG;
    let _: RegisterId<u32, test_block::control::Register> = test_block::control::REG;
    let _: RegisterId<u32, test_block::data::Register> = test_block::data::REG;
}

// ---------------------------------------------------------------------------
// Field constant tests
// ---------------------------------------------------------------------------

#[test]
fn fields_without_named_values_are_direct_constants() {
    // BUSY and ERROR have no named values -> direct Field constants
    let _: Field<u32, test_block::status::Register> = test_block::status::BUSY;
    let _: Field<u32, test_block::status::Register> = test_block::status::ERROR;
    let _: Field<u32, test_block::control::Register> = test_block::control::RESET;
    let _: Field<u32, test_block::data::Register> = test_block::data::VALUE;
    let _: Field<u32, test_block::data::Register> = test_block::data::TAG;
}

#[test]
fn fields_with_named_values_have_submodule_field() {
    // MODE has named values -> sub-module with FIELD constant
    let _: Field<u32, test_block::status::Register> = test_block::status::MODE::FIELD;
    let _: Field<u32, test_block::control::Register> = test_block::control::ENABLE::FIELD;
    let _: Field<u32, test_block::control::Register> = test_block::control::SPEED::FIELD;
}

#[test]
fn named_value_paths_exist() {
    // status::MODE::IDLE, status::MODE::ACTIVE, status::MODE::SLEEP
    let _: FieldValue<u32, test_block::status::Register> = test_block::status::MODE::IDLE;
    let _: FieldValue<u32, test_block::status::Register> = test_block::status::MODE::ACTIVE;
    let _: FieldValue<u32, test_block::status::Register> = test_block::status::MODE::SLEEP;

    // control::ENABLE::DISABLED, control::ENABLE::ENABLED
    let _: FieldValue<u32, test_block::control::Register> = test_block::control::ENABLE::DISABLED;
    let _: FieldValue<u32, test_block::control::Register> = test_block::control::ENABLE::ENABLED;

    // control::SPEED::SLOW, FAST, TURBO
    let _: FieldValue<u32, test_block::control::Register> = test_block::control::SPEED::SLOW;
    let _: FieldValue<u32, test_block::control::Register> = test_block::control::SPEED::FAST;
    let _: FieldValue<u32, test_block::control::Register> = test_block::control::SPEED::TURBO;
}

// ---------------------------------------------------------------------------
// Byte offset tests
// ---------------------------------------------------------------------------

#[test]
fn byte_offsets_are_sequential() {
    assert_eq!(test_block::status::OFFSET, 0);
    assert_eq!(test_block::control::OFFSET, 4);
    // _reserved0 occupies 2 * 4 = 8 bytes (offsets 8..16)
    assert_eq!(test_block::data::OFFSET, 16);
}

// ---------------------------------------------------------------------------
// RegisterOf trait tests
// ---------------------------------------------------------------------------

#[test]
fn register_of_impls_have_correct_access() {
    fn assert_read_access<Block, R: RegisterOf<Block>>()
    where
        R::Access: ReadAccess,
    {
    }
    fn assert_write_access<Block, R: RegisterOf<Block>>()
    where
        R::Access: WriteAccess,
    {
    }

    // status is ReadOnly -> ReadAccess only
    assert_read_access::<test_block::Regs, test_block::status::Register>();

    // control is ReadWrite -> both
    assert_read_access::<test_block::Regs, test_block::control::Register>();
    assert_write_access::<test_block::Regs, test_block::control::Register>();

    // data is ReadWrite -> both
    assert_read_access::<test_block::Regs, test_block::data::Register>();
    assert_write_access::<test_block::Regs, test_block::data::Register>();
}

// ---------------------------------------------------------------------------
// Mmio integration tests (type-check only)
// ---------------------------------------------------------------------------

#[test]
fn mmio_read_field_compiles_with_register_block() {
    fn assert_compiles(mmio: &Mmio<test_block::Regs>) {
        let _: u32 = mmio.read_field(test_block::status::BUSY);
    }
    let _ = assert_compiles;
}

#[test]
fn mmio_read_with_register_id_compiles() {
    fn assert_compiles(mmio: &Mmio<test_block::Regs>) {
        let _: u32 = mmio.read(test_block::data::REG);
    }
    let _ = assert_compiles;
}

#[test]
fn mmio_write_field_compiles_with_register_block() {
    fn assert_compiles(mmio: &Mmio<test_block::Regs>) {
        mmio.write_field(test_block::control::ENABLE::ENABLED);
    }
    let _ = assert_compiles;
}

#[test]
fn mmio_write_with_register_id_compiles() {
    fn assert_compiles(mmio: &Mmio<test_block::Regs>) {
        mmio.write(test_block::data::REG, 0x1234u32);
    }
    let _ = assert_compiles;
}

#[test]
fn mmio_modify_compiles_with_register_block() {
    fn assert_compiles(mmio: &Mmio<test_block::Regs>) {
        mmio.modify(
            test_block::control::REG,
            test_block::control::ENABLE::ENABLED,
        );
    }
    let _ = assert_compiles;
}

#[test]
fn field_value_composition_works_across_fields() {
    // Compose ENABLE::ENABLED + SPEED::FAST
    let combined = test_block::control::ENABLE::ENABLED + test_block::control::SPEED::FAST;
    let _: FieldValue<u32, test_block::control::Register> = combined;
}

// ---------------------------------------------------------------------------
// Layout coherence tests
// ---------------------------------------------------------------------------

#[test]
fn regs_struct_has_correct_size() {
    // status (4) + control (4) + reserved[2] (8) + data (4) = 20 bytes
    assert_eq!(core::mem::size_of::<test_block::Regs>(), 20);
}

// ---------------------------------------------------------------------------
// Field mask and shift correctness
// ---------------------------------------------------------------------------

#[test]
fn field_masks_are_correct() {
    // BUSY: OFFSET(0) NUMBITS(1) -> mask = 0x01, shift = 0
    assert_eq!(test_block::status::BUSY.mask, 0x01);
    assert_eq!(test_block::status::BUSY.shift, 0);

    // ERROR: OFFSET(1) NUMBITS(1) -> mask = 0x02, shift = 1
    assert_eq!(test_block::status::ERROR.mask, 0x02);
    assert_eq!(test_block::status::ERROR.shift, 1);

    // MODE: OFFSET(2) NUMBITS(2) -> mask = 0x0C, shift = 2
    assert_eq!(test_block::status::MODE::FIELD.mask, 0x0C);
    assert_eq!(test_block::status::MODE::FIELD.shift, 2);

    // VALUE: OFFSET(0) NUMBITS(16) -> mask = 0xFFFF, shift = 0
    assert_eq!(test_block::data::VALUE.mask, 0xFFFF);
    assert_eq!(test_block::data::VALUE.shift, 0);

    // TAG: OFFSET(16) NUMBITS(8) -> mask = 0x00FF0000, shift = 16
    assert_eq!(test_block::data::TAG.mask, 0x00FF_0000);
    assert_eq!(test_block::data::TAG.shift, 16);
}

#[test]
fn named_values_are_shifted_correctly() {
    // ENABLE::ENABLED: value=1, OFFSET(0) -> shifted value = 1
    assert_eq!(test_block::control::ENABLE::ENABLED.value, 1);
    assert_eq!(test_block::control::ENABLE::ENABLED.mask, 0x01);

    // SPEED::FAST: value=1, OFFSET(1) -> shifted value = 0b10 = 2
    assert_eq!(test_block::control::SPEED::FAST.value, 2);
    assert_eq!(test_block::control::SPEED::FAST.mask, 0x06); // NUMBITS(2) at OFFSET(1)

    // SPEED::TURBO: value=3, OFFSET(1) -> shifted value = 0b110 = 6
    assert_eq!(test_block::control::SPEED::TURBO.value, 6);
}

// ---------------------------------------------------------------------------
// Compile-fail documentation (negative bounds)
// ---------------------------------------------------------------------------

// A second block to test cross-block isolation.
regseq::register_block! {
    #[endian = "little", access_width = 32]
    pub other_block {
        reg_a: ReadWrite {
            BITS OFFSET(0) NUMBITS(8) [],
        },
    }
}

#[test]
fn write_to_readonly_register_should_not_compile() {
    // Uncommenting the body should fail because status::Register has
    // ReadOnlyAccess, which does not satisfy WriteAccess.
    //
    // fn bad(mmio: &Mmio<test_block::Regs>) {
    //     mmio.write(test_block::status::REG, 0u32);
    // }
    //
    // Similarly, write_field should be rejected:
    // fn bad2(mmio: &Mmio<test_block::Regs>) {
    //     mmio.write_field(test_block::status::MODE::IDLE);
    // }

    // Positive check: reading a ReadOnly register compiles.
    fn ok(mmio: &Mmio<test_block::Regs>) {
        let _: u32 = mmio.read(test_block::status::REG);
    }
    let _ = ok;
}

#[test]
fn cross_block_field_should_not_compile() {
    // Uncommenting the body should fail because other_block::reg_a::Register
    // implements RegisterOf<other_block::Regs>, not RegisterOf<test_block::Regs>.
    //
    // fn bad(mmio: &Mmio<test_block::Regs>) {
    //     mmio.read_field(other_block::reg_a::BITS);
    // }

    // Positive check: same-block field access compiles.
    fn ok(mmio: &Mmio<test_block::Regs>) {
        let _: u32 = mmio.read_field(test_block::status::BUSY);
    }
    let _ = ok;
}
