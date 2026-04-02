use regseq::{ReadOnly, ReadWrite, Reserved, WriteOnly};
use regseq_macros::register_bitfields;

register_bitfields! [
    u32,

    DR_REG [
        OE OFFSET(11) NUMBITS(1) [],
        BE OFFSET(10) NUMBITS(1) [],
        PE OFFSET(9) NUMBITS(1) [],
        FE OFFSET(8) NUMBITS(1) [],
        DATA OFFSET(0) NUMBITS(8) []
    ],

    RSR_ECR_REG [
        OE OFFSET(3) NUMBITS(1) [],
        BE OFFSET(2) NUMBITS(1) [],
        PE OFFSET(1) NUMBITS(1) [],
        FE OFFSET(0) NUMBITS(1) []
    ],

    FR_REG [
        RI OFFSET(8) NUMBITS(1) [],
        TXFE OFFSET(7) NUMBITS(1) [],
        RXFF OFFSET(6) NUMBITS(1) [],
        TXFF OFFSET(5) NUMBITS(1) [],
        RXFE OFFSET(4) NUMBITS(1) [],
        BUSY OFFSET(3) NUMBITS(1) [],
        DCD OFFSET(2) NUMBITS(1) [],
        DSR OFFSET(1) NUMBITS(1) [],
        CTS OFFSET(0) NUMBITS(1) []
    ],

    ILPR_REG [
        ILPDVSR OFFSET(0) NUMBITS(8) []
    ],

    IBRD_REG [
        BAUD_DIVINT OFFSET(0) NUMBITS(16) []
    ],

    FBRD_REG [
        BAUD_DIVFRAC OFFSET(0) NUMBITS(6) []
    ],

    LCRH_REG [
        SPS OFFSET(7) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ],
        WLEN OFFSET(5) NUMBITS(2) [
            FIVE_BITS = 0b00,
            SIX_BITS = 0b01,
            SEVEN_BITS = 0b10,
            EIGHT_BITS = 0b11
        ],
        FEN OFFSET(4) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ],
        STP2 OFFSET(3) NUMBITS(1) [
            ONE_STOP_BIT = 0,
            TWO_STOP_BITS = 1
        ],
        EPS OFFSET(2) NUMBITS(1) [
            ODD = 0,
            EVEN = 1
        ],
        PEN OFFSET(1) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ],
        BRK OFFSET(0) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ]
    ],

    CR_REG [
        CTSEN OFFSET(15) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ],
        RTSEN OFFSET(14) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ],
        OUT2 OFFSET(13) NUMBITS(1) [
            INACTIVE = 0,
            ACTIVE = 1
        ],
        OUT1 OFFSET(12) NUMBITS(1) [
            INACTIVE = 0,
            ACTIVE = 1
        ],
        RTS OFFSET(11) NUMBITS(1) [
            INACTIVE = 0,
            ACTIVE = 1
        ],
        DTR OFFSET(10) NUMBITS(1) [
            INACTIVE = 0,
            ACTIVE = 1
        ],
        RXE OFFSET(9) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ],
        TXE OFFSET(8) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ],
        LBE OFFSET(7) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ],
        SIRLP OFFSET(2) NUMBITS(1) [
            NORMAL = 0,
            LOW_POWER = 1
        ],
        SIREN OFFSET(1) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ],
        UARTEN OFFSET(0) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ]
    ],

    IFLS_REG [
        RXIFLSEL OFFSET(3) NUMBITS(3) [
            ONE_EIGHTH = 0b000,
            ONE_QUARTER = 0b001,
            ONE_HALF = 0b010,
            THREE_QUARTERS = 0b011,
            SEVEN_EIGHTHS = 0b100
        ],
        TXIFLSEL OFFSET(0) NUMBITS(3) [
            ONE_EIGHTH = 0b000,
            ONE_QUARTER = 0b001,
            ONE_HALF = 0b010,
            THREE_QUARTERS = 0b011,
            SEVEN_EIGHTHS = 0b100
        ]
    ],

    IMSC_REG [
        OEIM OFFSET(10) NUMBITS(1) [],
        BEIM OFFSET(9) NUMBITS(1) [],
        PEIM OFFSET(8) NUMBITS(1) [],
        FEIM OFFSET(7) NUMBITS(1) [],
        RTIM OFFSET(6) NUMBITS(1) [],
        TXIM OFFSET(5) NUMBITS(1) [],
        RXIM OFFSET(4) NUMBITS(1) [],
        DSRMIM OFFSET(3) NUMBITS(1) [],
        DCDMIM OFFSET(2) NUMBITS(1) [],
        CTSMIM OFFSET(1) NUMBITS(1) [],
        RIMIM OFFSET(0) NUMBITS(1) []
    ],

    RIS_REG [
        OERIS OFFSET(10) NUMBITS(1) [],
        BERIS OFFSET(9) NUMBITS(1) [],
        PERIS OFFSET(8) NUMBITS(1) [],
        FERIS OFFSET(7) NUMBITS(1) [],
        RTRIS OFFSET(6) NUMBITS(1) [],
        TXRIS OFFSET(5) NUMBITS(1) [],
        RXRIS OFFSET(4) NUMBITS(1) [],
        DSRRMIS OFFSET(3) NUMBITS(1) [],
        DCDRMIS OFFSET(2) NUMBITS(1) [],
        CTSRMIS OFFSET(1) NUMBITS(1) [],
        RIRMIS OFFSET(0) NUMBITS(1) []
    ],

    MIS_REG [
        OEMIS OFFSET(10) NUMBITS(1) [],
        BEMIS OFFSET(9) NUMBITS(1) [],
        PEMIS OFFSET(8) NUMBITS(1) [],
        FEMIS OFFSET(7) NUMBITS(1) [],
        RTMIS OFFSET(6) NUMBITS(1) [],
        TXMIS OFFSET(5) NUMBITS(1) [],
        RXMIS OFFSET(4) NUMBITS(1) [],
        DSRMMIS OFFSET(3) NUMBITS(1) [],
        DCDMMIS OFFSET(2) NUMBITS(1) [],
        CTSMMIS OFFSET(1) NUMBITS(1) [],
        RIMMIS OFFSET(0) NUMBITS(1) []
    ],

    ICR_REG [
        OEIC OFFSET(10) NUMBITS(1) [],
        BEIC OFFSET(9) NUMBITS(1) [],
        PEIC OFFSET(8) NUMBITS(1) [],
        FEIC OFFSET(7) NUMBITS(1) [],
        RTIC OFFSET(6) NUMBITS(1) [],
        TXIC OFFSET(5) NUMBITS(1) [],
        RXIC OFFSET(4) NUMBITS(1) [],
        DSRMIC OFFSET(3) NUMBITS(1) [],
        DCDMIC OFFSET(2) NUMBITS(1) [],
        CTSMIC OFFSET(1) NUMBITS(1) [],
        RIMIC OFFSET(0) NUMBITS(1) []
    ],

    DMACR_REG [
        DMAONERR OFFSET(2) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ],
        TXDMAE OFFSET(1) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ],
        RXDMAE OFFSET(0) NUMBITS(1) [
            DISABLED = 0,
            ENABLED = 1
        ]
    ],

    PERIPHID0_REG [
        PARTNUMBER0 OFFSET(0) NUMBITS(8) [
            PL011 = 0x11
        ]
    ],

    PERIPHID1_REG [
        DESIGNER0 OFFSET(4) NUMBITS(4) [
            ARM_LOW = 0x1
        ],
        PARTNUMBER1 OFFSET(0) NUMBITS(4) [
            PL011 = 0x0
        ]
    ],

    PERIPHID2_REG [
        REVISION OFFSET(4) NUMBITS(4) [
            R1P0 = 0x0,
            R1P1 = 0x1,
            R1P3 = 0x2,
            R1P4 = 0x2,
            R1P5 = 0x3
        ],
        DESIGNER1 OFFSET(0) NUMBITS(4) [
            ARM_HIGH = 0x4
        ]
    ],

    PERIPHID3_REG [
        CONFIGURATION OFFSET(0) NUMBITS(8) [
            DEFAULT = 0x00
        ]
    ],

    PCELLID0_REG [
        VALUE OFFSET(0) NUMBITS(8) [
            DEFAULT = 0x0D
        ]
    ],

    PCELLID1_REG [
        VALUE OFFSET(0) NUMBITS(8) [
            DEFAULT = 0xF0
        ]
    ],

    PCELLID2_REG [
        VALUE OFFSET(0) NUMBITS(8) [
            DEFAULT = 0x05
        ]
    ],

    PCELLID3_REG [
        VALUE OFFSET(0) NUMBITS(8) [
            DEFAULT = 0xB1
        ]
    ]
];

/// Full PL011 register block for the evolving full-spec fixture.
///
/// This models the architected register layout through the DMA and
/// identification registers using 32-bit accessors and explicit reserved gaps.
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct Pl011Registers {
    /// 0x000 - Data Register.
    pub dr: ReadWrite<u32, DR_REG::Register>,

    /// 0x004 - Receive Status / Error Clear Register.
    pub rsr_ecr: ReadWrite<u32, RSR_ECR_REG::Register>,

    /// 0x008..0x014 - Reserved.
    _reserved0: [Reserved<u32>; 4],

    /// 0x018 - Flag Register.
    pub fr: ReadOnly<u32, FR_REG::Register>,

    /// 0x01C - Reserved.
    _reserved1: Reserved<u32>,

    /// 0x020 - IrDA Low-Power Counter Register.
    pub ilpr: ReadWrite<u32, ILPR_REG::Register>,

    /// 0x024 - Integer Baud Rate Register.
    pub ibrd: ReadWrite<u32, IBRD_REG::Register>,

    /// 0x028 - Fractional Baud Rate Register.
    pub fbrd: ReadWrite<u32, FBRD_REG::Register>,

    /// 0x02C - Line Control Register.
    pub lcr_h: ReadWrite<u32, LCRH_REG::Register>,

    /// 0x030 - Control Register.
    pub cr: ReadWrite<u32, CR_REG::Register>,

    /// 0x034 - Interrupt FIFO Level Select Register.
    pub ifls: ReadWrite<u32, IFLS_REG::Register>,

    /// 0x038 - Interrupt Mask Set/Clear Register.
    pub imsc: ReadWrite<u32, IMSC_REG::Register>,

    /// 0x03C - Raw Interrupt Status Register.
    pub ris: ReadOnly<u32, RIS_REG::Register>,

    /// 0x040 - Masked Interrupt Status Register.
    pub mis: ReadOnly<u32, MIS_REG::Register>,

    /// 0x044 - Interrupt Clear Register.
    pub icr: WriteOnly<u32, ICR_REG::Register>,

    /// 0x048 - DMA Control Register.
    pub dmacr: ReadWrite<u32, DMACR_REG::Register>,

    /// 0x04C..0xFDC - Reserved and test-only regions.
    _reserved2: [Reserved<u32>; 997],

    /// 0xFE0 - Peripheral Identification Register 0.
    pub periph_id0: ReadOnly<u32, PERIPHID0_REG::Register>,

    /// 0xFE4 - Peripheral Identification Register 1.
    pub periph_id1: ReadOnly<u32, PERIPHID1_REG::Register>,

    /// 0xFE8 - Peripheral Identification Register 2.
    pub periph_id2: ReadOnly<u32, PERIPHID2_REG::Register>,

    /// 0xFEC - Peripheral Identification Register 3.
    pub periph_id3: ReadOnly<u32, PERIPHID3_REG::Register>,

    /// 0xFF0 - PrimeCell Identification Register 0.
    pub pcell_id0: ReadOnly<u32, PCELLID0_REG::Register>,

    /// 0xFF4 - PrimeCell Identification Register 1.
    pub pcell_id1: ReadOnly<u32, PCELLID1_REG::Register>,

    /// 0xFF8 - PrimeCell Identification Register 2.
    pub pcell_id2: ReadOnly<u32, PCELLID2_REG::Register>,

    /// 0xFFC - PrimeCell Identification Register 3.
    pub pcell_id3: ReadOnly<u32, PCELLID3_REG::Register>,
}
