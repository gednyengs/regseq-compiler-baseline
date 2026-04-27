use crate::regseq::register_block;

register_block! {
    #[endian = "little", access_width = 32]
    pub pl011 {
        // 0x000 - Data Register
        dr: ReadWrite {
            DATA OFFSET(0) NUMBITS(8) [],
            FE OFFSET(8) NUMBITS(1) [],
            PE OFFSET(9) NUMBITS(1) [],
            BE OFFSET(10) NUMBITS(1) [],
            OE OFFSET(11) NUMBITS(1) [],
        },

        // 0x004 - Receive Status / Error Clear Register
        rsr_ecr: ReadWrite {
            FE OFFSET(0) NUMBITS(1) [],
            PE OFFSET(1) NUMBITS(1) [],
            BE OFFSET(2) NUMBITS(1) [],
            OE OFFSET(3) NUMBITS(1) [],
        },

        // 0x008..0x014 - Reserved
        _reserved0: Reserved<u32> [4],

        // 0x018 - Flag Register
        fr: ReadOnly {
            CTS OFFSET(0) NUMBITS(1) [],
            DSR OFFSET(1) NUMBITS(1) [],
            DCD OFFSET(2) NUMBITS(1) [],
            BUSY OFFSET(3) NUMBITS(1) [],
            RXFE OFFSET(4) NUMBITS(1) [],
            TXFF OFFSET(5) NUMBITS(1) [],
            RXFF OFFSET(6) NUMBITS(1) [],
            TXFE OFFSET(7) NUMBITS(1) [],
            RI OFFSET(8) NUMBITS(1) [],
        },

        // 0x01C - Reserved
        _reserved1: Reserved<u32> [1],

        // 0x020 - IrDA Low-Power Counter Register
        ilpr: ReadWrite {
            ILPDVSR OFFSET(0) NUMBITS(8) [],
        },

        // 0x024 - Integer Baud Rate Register
        ibrd: ReadWrite {
            BAUD_DIVINT OFFSET(0) NUMBITS(16) [],
        },

        // 0x028 - Fractional Baud Rate Register
        fbrd: ReadWrite {
            BAUD_DIVFRAC OFFSET(0) NUMBITS(6) [],
        },

        // 0x02C - Line Control Register
        lcr_h: ReadWrite {
            BRK OFFSET(0) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            PEN OFFSET(1) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            EPS OFFSET(2) NUMBITS(1) [
                ODD = 0,
                EVEN = 1,
            ],
            STP2 OFFSET(3) NUMBITS(1) [
                ONE_STOP_BIT = 0,
                TWO_STOP_BITS = 1,
            ],
            FEN OFFSET(4) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            WLEN OFFSET(5) NUMBITS(2) [
                FIVE_BITS = 0,
                SIX_BITS = 1,
                SEVEN_BITS = 2,
                EIGHT_BITS = 3,
            ],
            SPS OFFSET(7) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
        },

        // 0x030 - Control Register
        cr: ReadWrite {
            UARTEN OFFSET(0) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            SIREN OFFSET(1) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            SIRLP OFFSET(2) NUMBITS(1) [
                NORMAL = 0,
                LOW_POWER = 1,
            ],
            LBE OFFSET(7) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            TXE OFFSET(8) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            RXE OFFSET(9) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            DTR OFFSET(10) NUMBITS(1) [
                INACTIVE = 0,
                ACTIVE = 1,
            ],
            RTS OFFSET(11) NUMBITS(1) [
                INACTIVE = 0,
                ACTIVE = 1,
            ],
            OUT1 OFFSET(12) NUMBITS(1) [
                INACTIVE = 0,
                ACTIVE = 1,
            ],
            OUT2 OFFSET(13) NUMBITS(1) [
                INACTIVE = 0,
                ACTIVE = 1,
            ],
            RTSEN OFFSET(14) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            CTSEN OFFSET(15) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
        },

        // 0x034 - Interrupt FIFO Level Select Register
        ifls: ReadWrite {
            TXIFLSEL OFFSET(0) NUMBITS(3) [
                ONE_EIGHTH = 0,
                ONE_QUARTER = 1,
                ONE_HALF = 2,
                THREE_QUARTERS = 3,
                SEVEN_EIGHTHS = 4,
            ],
            RXIFLSEL OFFSET(3) NUMBITS(3) [
                ONE_EIGHTH = 0,
                ONE_QUARTER = 1,
                ONE_HALF = 2,
                THREE_QUARTERS = 3,
                SEVEN_EIGHTHS = 4,
            ],
        },

        // 0x038 - Interrupt Mask Set/Clear Register
        imsc: ReadWrite {
            RIMIM OFFSET(0) NUMBITS(1) [],
            CTSMIM OFFSET(1) NUMBITS(1) [],
            DCDMIM OFFSET(2) NUMBITS(1) [],
            DSRMIM OFFSET(3) NUMBITS(1) [],
            RXIM OFFSET(4) NUMBITS(1) [],
            TXIM OFFSET(5) NUMBITS(1) [],
            RTIM OFFSET(6) NUMBITS(1) [],
            FEIM OFFSET(7) NUMBITS(1) [],
            PEIM OFFSET(8) NUMBITS(1) [],
            BEIM OFFSET(9) NUMBITS(1) [],
            OEIM OFFSET(10) NUMBITS(1) [],
        },

        // 0x03C - Raw Interrupt Status Register
        ris: ReadOnly {
            RIRMIS OFFSET(0) NUMBITS(1) [],
            CTSRMIS OFFSET(1) NUMBITS(1) [],
            DCDRMIS OFFSET(2) NUMBITS(1) [],
            DSRRMIS OFFSET(3) NUMBITS(1) [],
            RXRIS OFFSET(4) NUMBITS(1) [],
            TXRIS OFFSET(5) NUMBITS(1) [],
            RTRIS OFFSET(6) NUMBITS(1) [],
            FERIS OFFSET(7) NUMBITS(1) [],
            PERIS OFFSET(8) NUMBITS(1) [],
            BERIS OFFSET(9) NUMBITS(1) [],
            OERIS OFFSET(10) NUMBITS(1) [],
        },

        // 0x040 - Masked Interrupt Status Register
        mis: ReadOnly {
            RIMMIS OFFSET(0) NUMBITS(1) [],
            CTSMMIS OFFSET(1) NUMBITS(1) [],
            DCDMMIS OFFSET(2) NUMBITS(1) [],
            DSRMMIS OFFSET(3) NUMBITS(1) [],
            RXMIS OFFSET(4) NUMBITS(1) [],
            TXMIS OFFSET(5) NUMBITS(1) [],
            RTMIS OFFSET(6) NUMBITS(1) [],
            FEMIS OFFSET(7) NUMBITS(1) [],
            PEMIS OFFSET(8) NUMBITS(1) [],
            BEMIS OFFSET(9) NUMBITS(1) [],
            OEMIS OFFSET(10) NUMBITS(1) [],
        },

        // 0x044 - Interrupt Clear Register
        icr: WriteOnly {
            RIMIC OFFSET(0) NUMBITS(1) [],
            CTSMIC OFFSET(1) NUMBITS(1) [],
            DCDMIC OFFSET(2) NUMBITS(1) [],
            DSRMIC OFFSET(3) NUMBITS(1) [],
            RXIC OFFSET(4) NUMBITS(1) [],
            TXIC OFFSET(5) NUMBITS(1) [],
            RTIC OFFSET(6) NUMBITS(1) [],
            FEIC OFFSET(7) NUMBITS(1) [],
            PEIC OFFSET(8) NUMBITS(1) [],
            BEIC OFFSET(9) NUMBITS(1) [],
            OEIC OFFSET(10) NUMBITS(1) [],
        },

        // 0x048 - DMA Control Register
        dmacr: ReadWrite {
            RXDMAE OFFSET(0) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            TXDMAE OFFSET(1) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            DMAONERR OFFSET(2) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
        },

        // 0x04C..0xFDC - Reserved and test-only regions
        _reserved2: Reserved<u32> [997],

        // 0xFE0 - Peripheral Identification Register 0
        periph_id0: ReadOnly {
            PARTNUMBER0 OFFSET(0) NUMBITS(8) [
                PL011 = 17,
            ],
        },

        // 0xFE4 - Peripheral Identification Register 1
        periph_id1: ReadOnly {
            PARTNUMBER1 OFFSET(0) NUMBITS(4) [
                PL011 = 0,
            ],
            DESIGNER0 OFFSET(4) NUMBITS(4) [
                ARM_LOW = 1,
            ],
        },

        // 0xFE8 - Peripheral Identification Register 2
        periph_id2: ReadOnly {
            DESIGNER1 OFFSET(0) NUMBITS(4) [
                ARM_HIGH = 4,
            ],
            REVISION OFFSET(4) NUMBITS(4) [
                R1P0 = 0,
                R1P1 = 1,
                R1P3 = 2,
                R1P4 = 2,
                R1P5 = 3,
            ],
        },

        // 0xFEC - Peripheral Identification Register 3
        periph_id3: ReadOnly {
            CONFIGURATION OFFSET(0) NUMBITS(8) [
                DEFAULT = 0,
            ],
        },

        // 0xFF0 - PrimeCell Identification Register 0
        pcell_id0: ReadOnly {
            VALUE OFFSET(0) NUMBITS(8) [
                DEFAULT = 13,
            ],
        },

        // 0xFF4 - PrimeCell Identification Register 1
        pcell_id1: ReadOnly {
            VALUE OFFSET(0) NUMBITS(8) [
                DEFAULT = 240,
            ],
        },

        // 0xFF8 - PrimeCell Identification Register 2
        pcell_id2: ReadOnly {
            VALUE OFFSET(0) NUMBITS(8) [
                DEFAULT = 5,
            ],
        },

        // 0xFFC - PrimeCell Identification Register 3
        pcell_id3: ReadOnly {
            VALUE OFFSET(0) NUMBITS(8) [
                DEFAULT = 177,
            ],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::pl011;

    #[test]
    fn regs_type_exists() {
        // Verify the Regs layout type is generated.
        let _size = core::mem::size_of::<pl011::Regs>();
    }

    #[test]
    fn register_modules_exist() {
        // Verify key register modules and their REG/OFFSET constants.
        let _ = pl011::dr::REG;
        let _ = pl011::dr::OFFSET;
        let _ = pl011::rsr_ecr::REG;
        let _ = pl011::fr::REG;
        let _ = pl011::ilpr::REG;
        let _ = pl011::ibrd::REG;
        let _ = pl011::fbrd::REG;
        let _ = pl011::lcr_h::REG;
        let _ = pl011::cr::REG;
        let _ = pl011::ifls::REG;
        let _ = pl011::imsc::REG;
        let _ = pl011::ris::REG;
        let _ = pl011::mis::REG;
        let _ = pl011::icr::REG;
        let _ = pl011::dmacr::REG;
        let _ = pl011::periph_id0::REG;
        let _ = pl011::periph_id1::REG;
        let _ = pl011::periph_id2::REG;
        let _ = pl011::periph_id3::REG;
        let _ = pl011::pcell_id0::REG;
        let _ = pl011::pcell_id1::REG;
        let _ = pl011::pcell_id2::REG;
        let _ = pl011::pcell_id3::REG;
    }

    #[test]
    fn field_constants_exist() {
        // Verify field constants are accessible.
        let _ = pl011::dr::DATA;
        let _ = pl011::dr::FE;
        let _ = pl011::fr::BUSY;
        let _ = pl011::fr::TXFE;
        let _ = pl011::ibrd::BAUD_DIVINT;
        let _ = pl011::imsc::RXIM;
        let _ = pl011::imsc::TXIM;
    }

    #[test]
    fn named_values_exist() {
        // Verify named values on fields with enumerated options.
        let _ = pl011::cr::UARTEN::ENABLED;
        let _ = pl011::cr::UARTEN::DISABLED;
        let _ = pl011::cr::TXE::ENABLED;
        let _ = pl011::cr::RXE::ENABLED;
        let _ = pl011::lcr_h::WLEN::EIGHT_BITS;
        let _ = pl011::lcr_h::WLEN::FIVE_BITS;
        let _ = pl011::lcr_h::FEN::ENABLED;
        let _ = pl011::lcr_h::STP2::TWO_STOP_BITS;
        let _ = pl011::ifls::TXIFLSEL::ONE_HALF;
        let _ = pl011::ifls::RXIFLSEL::SEVEN_EIGHTHS;
        let _ = pl011::dmacr::RXDMAE::ENABLED;
        let _ = pl011::periph_id0::PARTNUMBER0::PL011;
    }

    #[test]
    fn byte_offsets_match_hardware_spec() {
        // Verify byte offsets match the PL011 hardware specification.
        assert_eq!(pl011::dr::OFFSET, 0x000);
        assert_eq!(pl011::rsr_ecr::OFFSET, 0x004);
        assert_eq!(pl011::fr::OFFSET, 0x018);
        assert_eq!(pl011::ilpr::OFFSET, 0x020);
        assert_eq!(pl011::ibrd::OFFSET, 0x024);
        assert_eq!(pl011::fbrd::OFFSET, 0x028);
        assert_eq!(pl011::lcr_h::OFFSET, 0x02C);
        assert_eq!(pl011::cr::OFFSET, 0x030);
        assert_eq!(pl011::ifls::OFFSET, 0x034);
        assert_eq!(pl011::imsc::OFFSET, 0x038);
        assert_eq!(pl011::ris::OFFSET, 0x03C);
        assert_eq!(pl011::mis::OFFSET, 0x040);
        assert_eq!(pl011::icr::OFFSET, 0x044);
        assert_eq!(pl011::dmacr::OFFSET, 0x048);
        assert_eq!(pl011::periph_id0::OFFSET, 0xFE0);
        assert_eq!(pl011::periph_id1::OFFSET, 0xFE4);
        assert_eq!(pl011::periph_id2::OFFSET, 0xFE8);
        assert_eq!(pl011::periph_id3::OFFSET, 0xFEC);
        assert_eq!(pl011::pcell_id0::OFFSET, 0xFF0);
        assert_eq!(pl011::pcell_id1::OFFSET, 0xFF4);
        assert_eq!(pl011::pcell_id2::OFFSET, 0xFF8);
        assert_eq!(pl011::pcell_id3::OFFSET, 0xFFC);
    }
}
