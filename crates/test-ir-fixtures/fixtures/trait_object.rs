#![no_std]
pub trait Reader {
    fn read_byte(&self) -> u8;
}
pub fn drive(r: &dyn Reader) -> u8 {
    r.read_byte()
}
