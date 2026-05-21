#![no_std]
pub fn boom() -> ! {
    loop {}
}
pub fn entry() {
    boom()
}
