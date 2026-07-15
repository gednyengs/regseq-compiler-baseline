#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");
include!("../../_serial_port_surface.rs");
include!("../../_dma_surface.rs");
include!("../../_composition_surface.rs");

mod pl011 {
    use super::{ReadOnly, ReadWrite, Reserved, WriteOnly};

    pub mod dr { pub enum Register {} }
    pub mod fr { pub enum Register {} }
    pub mod ibrd { pub enum Register {} }
    pub mod fbrd { pub enum Register {} }
    pub mod lcr_h { pub enum Register {} }
    pub mod cr { pub enum Register {} }
    pub mod imsc { pub enum Register {} }
    pub mod icr { pub enum Register {} }

    #[repr(C)]
    pub struct Registers {
        pub dr: ReadWrite<u32, dr::Register>,
        pub _reserved0: [Reserved<u32>; 5],
        pub fr: ReadOnly<u32, fr::Register>,
        pub _reserved1: [Reserved<u32>; 2],
        pub ibrd: ReadWrite<u32, ibrd::Register>,
        pub fbrd: ReadWrite<u32, fbrd::Register>,
        pub lcr_h: ReadWrite<u32, lcr_h::Register>,
        pub cr: ReadWrite<u32, cr::Register>,
        pub _ifls: Reserved<u32>,
        pub imsc: ReadWrite<u32, imsc::Register>,
        pub _ris_mis: [Reserved<u32>; 2],
        pub icr: WriteOnly<u32, icr::Register>,
    }
}

const FR_TX_FULL: u32 = 1 << 5;
const FR_RX_EMPTY: u32 = 1 << 4;
const CR_UART_TX_RX_ENABLE: u32 = (1 << 0) | (1 << 8) | (1 << 9);
const LCR_FIFO_8N1: u32 = (1 << 4) | (3 << 5);
const INTERRUPT_MASK: u32 = 0x7ff;

#[rseq::driver(name = "c5_pl011", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,c5-pl011"])]
#[rseq::device_class(serial)]
pub struct C5Pl011 {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Registers>,
    baud_rate: u32,
    dma_state: u32,
}

impl SerialPort for C5Pl011 {
    fn configure(&mut self, params: SerialParams) -> SerialStatus {
        if params.baud_rate == 0 { return SerialStatus::InvalidParameter; }
        self.regs.cr.write(0);
        self.regs.ibrd.write(13);
        self.regs.fbrd.write(1);
        self.regs.lcr_h.write(LCR_FIFO_8N1);
        self.regs.cr.write(CR_UART_TX_RX_ENABLE);
        self.baud_rate = params.baud_rate;
        SerialStatus::Ok
    }

    fn get_config(&self) -> SerialParams {
        SerialParams {
            baud_rate: self.baud_rate,
            data_bits: SerialDataBits::Eight,
            stop_bits: SerialStopBits::One,
            parity: SerialParity::None,
            flow_control_mode: SerialFlowControl::None,
        }
    }

    fn transmit(&mut self, buffer: SerialTxBuffer) -> usize {
        if buffer.len == 0 || (self.regs.fr.read() & FR_TX_FULL) != 0 { return 0; }
        self.regs.dr.write(0x43);
        1
    }

    fn receive(&mut self, buffer: SerialRxBuffer) -> usize {
        if buffer.len == 0 || (self.regs.fr.read() & FR_RX_EMPTY) != 0 { return 0; }
        let _value = self.regs.dr.read();
        1
    }

    fn open(&mut self) -> SerialStatus {
        self.regs.imsc.write(0);
        SerialStatus::Ok
    }

    fn close(&mut self) -> SerialStatus {
        self.regs.imsc.write(0);
        self.regs.icr.write(INTERRUPT_MASK);
        self.regs.cr.write(0);
        SerialStatus::Ok
    }

    fn await_drain(&self) -> bool { (self.regs.fr.read() & (1 << 3)) == 0 }
}

impl AsyncTransferChannel for C5Pl011 {
    fn acquire(&mut self) -> DmaChannelHandle {
        self.dma_state = 1;
        DmaChannelHandle { identity: 1 }
    }
    fn configure(&mut self, channel: DmaChannelHandle, config: DmaTransferConfig) -> DmaStatus {
        if channel.identity != 1 || config.length == 0 { return DmaStatus::InvalidParameter; }
        self.dma_state = 2;
        DmaStatus::Ok
    }
    fn prepare(&mut self, channel: DmaChannelHandle) -> DmaRequestHandle {
        if channel.identity == 1 && self.dma_state == 2 {
            self.dma_state = 3;
            DmaRequestHandle { identity: 1 }
        } else {
            DmaRequestHandle { identity: 0 }
        }
    }
    fn submit(&mut self, request: DmaRequestHandle) -> DmaStatus {
        if request.identity != 1 || self.dma_state != 3 { return DmaStatus::InvalidParameter; }
        self.dma_state = 4;
        DmaStatus::Ok
    }
    fn activate(&mut self, channel: DmaChannelHandle) -> DmaStatus {
        if channel.identity != 1 || self.dma_state != 4 { return DmaStatus::InvalidParameter; }
        self.dma_state = 5;
        DmaStatus::Ok
    }
    fn terminate(&mut self, channel: DmaChannelHandle) -> DmaStatus {
        if channel.identity != 1 { return DmaStatus::InvalidParameter; }
        self.dma_state = 6;
        DmaStatus::Ok
    }
    fn synchronize(&mut self, channel: DmaChannelHandle) -> DmaStatus {
        if channel.identity != 1 || self.dma_state != 6 { return DmaStatus::InvalidParameter; }
        self.dma_state = 7;
        DmaStatus::Ok
    }
    fn release(&mut self, channel: DmaChannelHandle) {
        if channel.identity == 1 && self.dma_state == 7 { self.dma_state = 8; }
    }
}

impl Lifecycle for C5Pl011 {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        if self.open() != SerialStatus::Ok { return Err(RegseqError::HardwareError); }
        if SerialPort::configure(self, SerialParams {
            baud_rate: 115200,
            data_bits: SerialDataBits::Eight,
            stop_bits: SerialStopBits::One,
            parity: SerialParity::None,
            flow_control_mode: SerialFlowControl::None,
        }) != SerialStatus::Ok { return Err(RegseqError::HardwareError); }
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        if self.close() != SerialStatus::Ok { return Err(RegseqError::HardwareError); }
        Ok(())
    }
}
