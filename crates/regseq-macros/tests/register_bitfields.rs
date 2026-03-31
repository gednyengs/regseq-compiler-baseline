use regseq::{Field, FieldValue, Mmio};
use regseq_macros::register_bitfields;

register_bitfields! [
    u32,

    DR_REG [
        DATA OFFSET(0) NUMBITS(8) []
    ],

    FR_REG [
        TXFF OFFSET(5) NUMBITS(1) [],
        BUSY OFFSET(3) NUMBITS(1) []
    ],

    LCRH_REG [
        WLEN OFFSET(5) NUMBITS(2) [
            WLEN_5 = 0b00,
            WLEN_6 = 0b01,
            WLEN_7 = 0b10,
            WLEN_8 = 0b11
        ],
        FEN OFFSET(4) NUMBITS(1) [
            SET = 1,
            CLEAR = 0
        ]
    ]
];

#[test]
fn macro_is_exported() {
    // The register_bitfields! invocation above compiled, confirming the macro
    // is exported by regseq-macros.
}

#[test]
fn register_layout_compiles() {
    let _: Field<u32, DR_REG::Register> = DR_REG::DATA::FIELD;
    let _: Field<u32, FR_REG::Register> = FR_REG::TXFF::FIELD;
    let _: Field<u32, FR_REG::Register> = FR_REG::BUSY::FIELD;
    let _: Field<u32, LCRH_REG::Register> = LCRH_REG::WLEN::FIELD;
    let _: Field<u32, LCRH_REG::Register> = LCRH_REG::FEN::FIELD;
}

#[test]
fn marker_types_are_distinct() {
    fn _takes_dr(_: DR_REG::Register) {}
    fn _takes_fr(_: FR_REG::Register) {}
    fn _takes_lcrh(_: LCRH_REG::Register) {}
}

#[test]
fn field_constants_have_correct_values() {
    let f = FR_REG::BUSY::FIELD;
    assert_eq!(f.mask, 1 << 3);
    assert_eq!(f.shift, 3);

    let f = FR_REG::TXFF::FIELD;
    assert_eq!(f.mask, 1 << 5);
    assert_eq!(f.shift, 5);

    let f = DR_REG::DATA::FIELD;
    assert_eq!(f.mask, 0xFF);
    assert_eq!(f.shift, 0);

    let f = LCRH_REG::WLEN::FIELD;
    assert_eq!(f.mask, 0b11 << 5);
    assert_eq!(f.shift, 5);
}

#[test]
fn named_value_constants_accessible() {
    let v: FieldValue<u32, LCRH_REG::Register> = LCRH_REG::WLEN::WLEN_8;
    assert_eq!(v.mask, 0b11 << 5);
    assert_eq!(v.value, 0b11 << 5);

    let v = LCRH_REG::WLEN::WLEN_5;
    assert_eq!(v.value, 0);

    let v = LCRH_REG::FEN::SET;
    assert_eq!(v.mask, 1 << 4);
    assert_eq!(v.value, 1 << 4);

    let v = LCRH_REG::FEN::CLEAR;
    assert_eq!(v.value, 0);
}

#[test]
fn field_value_composition() {
    let combined = LCRH_REG::WLEN::WLEN_8 + LCRH_REG::FEN::SET;
    assert_eq!(combined.mask, (0b11 << 5) | (1 << 4));
    assert_eq!(combined.value, (0b11 << 5) | (1 << 4));
}

#[test]
fn usable_as_mmio_target() {
    #[repr(C)]
    struct MockUart {
        pub dr: regseq::ReadWrite<u32, DR_REG::Register>,
        pub fr: regseq::ReadOnly<u32, FR_REG::Register>,
    }

    let _mmio: Mmio<MockUart> = Mmio::new(unsafe { core::mem::zeroed() });
}
