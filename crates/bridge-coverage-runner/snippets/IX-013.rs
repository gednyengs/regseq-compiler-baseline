#![no_std]
pub struct R;
impl Drop for R {
    fn drop(&mut self) {}
}
pub fn entry() {
    let r = R;
    let c = move || {
        let _captured = &r;
    };
    let _ = c;
}
