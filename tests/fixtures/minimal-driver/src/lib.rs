#[rseq::driver(name = "minimal")]
pub struct MinimalDriver {
    pub base_addr: usize,
}
