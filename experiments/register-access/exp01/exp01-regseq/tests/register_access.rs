use exp01_regseq::{
    poll, register_bitfields, Duration, Mmio, ReadOnly, ReadWrite, RegseqError, Reserved,
    WriteOnly,
};

register_bitfields! [
    u32,

    FR_REG [
        TXFF OFFSET(5) NUMBITS(1) [],
        BUSY OFFSET(3) NUMBITS(1) []
    ],

    LCRH_REG [
        WLEN OFFSET(5) NUMBITS(2) [
            WLEN_5 = 0,
            WLEN_6 = 1,
            WLEN_7 = 2,
            WLEN_8 = 3
        ],
        FEN OFFSET(4) NUMBITS(1) [
            CLEAR = 0,
            SET = 1
        ]
    ],

    INT_REG [
        RX OFFSET(4) NUMBITS(1) [
            CLEAR = 0,
            SET = 1
        ]
    ]
];

#[repr(C)]
pub struct Pl011Registers {
    pub fr: ReadOnly<u32, FR_REG::Register>,
    pub lcr_h: ReadWrite<u32, LCRH_REG::Register>,
    _reserved0: [Reserved<u32>; 2],
    pub icr: WriteOnly<u32, INT_REG::Register>,
}

fn configure_uart(regs: &Mmio<Pl011Registers>) {
    regs.lcr_h
        .modify(LCRH_REG::WLEN::WLEN_8 + LCRH_REG::FEN::SET);
}

fn wait_for_idle(regs: &Mmio<Pl011Registers>) -> Result<(), RegseqError> {
    poll::until(
        || regs.fr.read(FR_REG::BUSY::FIELD) == 0,
        Duration::from_us(10),
        Duration::from_ms(100),
    )
}

#[test]
fn mmio_deref_enables_field_access() {
    let regs = Mmio::new(Pl011Registers {
        fr: unsafe { core::mem::zeroed() },
        lcr_h: unsafe { core::mem::zeroed() },
        _reserved0: unsafe { core::mem::zeroed() },
        icr: unsafe { core::mem::zeroed() },
    });

    let _ = &regs.lcr_h;
    let _ = configure_uart;
    let _ = wait_for_idle;
}

#[test]
fn named_values_are_field_values() {
    let _combo = LCRH_REG::WLEN::WLEN_8 + LCRH_REG::FEN::SET;
}

