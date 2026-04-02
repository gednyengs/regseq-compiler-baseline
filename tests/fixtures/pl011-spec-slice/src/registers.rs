use regseq::{ReadOnly, ReadWrite, Reserved};
use regseq_macros::register_bitfields;

register_bitfields! [
    u32,

    DR_REG [
        DATA OFFSET(0) NUMBITS(8) []
    ],

    FR_REG [
        TXFF OFFSET(5) NUMBITS(1) [],
        RXFE OFFSET(4) NUMBITS(1) [],
        BUSY OFFSET(3) NUMBITS(1) []
    ],

    IBRD_REG [
        BAUD_DIVINT OFFSET(0) NUMBITS(16) []
    ],

    FBRD_REG [
        BAUD_DIVFRAC OFFSET(0) NUMBITS(6) []
    ],

    LCRH_REG [
        WLEN OFFSET(5) NUMBITS(2) [
            WLEN_8 = 0b11
        ],
        FEN OFFSET(4) NUMBITS(1) [
            SET = 1,
            CLEAR = 0
        ],
        BRK OFFSET(0) NUMBITS(1) [
            SET = 1,
            CLEAR = 0
        ]
    ],

    CR_REG [
        UARTEN OFFSET(0) NUMBITS(1) [
            SET = 1,
            CLEAR = 0
        ],
        TXE OFFSET(8) NUMBITS(1) [
            SET = 1,
            CLEAR = 0
        ],
        RXE OFFSET(9) NUMBITS(1) [
            SET = 1,
            CLEAR = 0
        ]
    ]
];

/// PL011 register block for the polling-first bring-up slice.
///
/// Layout follows the real PL011 offsets for the subset needed by basic UART
/// bring-up. Intervening gaps are modeled with `Reserved<u32>`.
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct Pl011Registers {
    /// 0x000 - Data Register
    pub dr: ReadWrite<u32, DR_REG::Register>,

    /// 0x004..0x014 - Reserved
    _reserved0: [Reserved<u32>; 5],

    /// 0x018 - Flag Register
    pub fr: ReadOnly<u32, FR_REG::Register>,

    /// 0x01C..0x020 - Reserved
    _reserved1: [Reserved<u32>; 2],

    /// 0x024 - Integer Baud Rate Register
    pub ibrd: ReadWrite<u32, IBRD_REG::Register>,

    /// 0x028 - Fractional Baud Rate Register
    pub fbrd: ReadWrite<u32, FBRD_REG::Register>,

    /// 0x02C - Line Control Register
    pub lcr_h: ReadWrite<u32, LCRH_REG::Register>,

    /// 0x030 - Control Register
    pub cr: ReadWrite<u32, CR_REG::Register>,
}
