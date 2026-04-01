#[rseq::driver(name = "property-test")]
pub struct PropertyTestDriver {
    #[rseq::property(id = "reg-io-width", default = 4)]
    pub reg_io_width: u32,

    #[rseq::property(id = "current-speed")]
    pub current_speed: Option<u32>,

    #[rseq::property(id = "fifo-size", default = 16)]
    pub fifo_size: u32,
}
