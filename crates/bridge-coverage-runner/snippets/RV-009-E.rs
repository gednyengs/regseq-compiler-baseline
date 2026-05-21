#![no_std]
pub union U {
    pub x: u32,
    pub y: f32,
}
pub fn f() -> u32 {
    let u = U { x: 0 };
    unsafe { u.x }
}
