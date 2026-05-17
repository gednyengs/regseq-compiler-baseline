#![no_std]
pub trait Reader {
    fn read_byte(&self) -> u8;
}
pub fn drive(r: &dyn Reader) -> u8 {
    r.read_byte()
}
pub struct StaticByte(pub u8);
impl Reader for StaticByte {
    fn read_byte(&self) -> u8 {
        self.0
    }
}
pub fn make_and_drive() -> u8 {
    let r = StaticByte(0x42);
    drive(&r)
}
