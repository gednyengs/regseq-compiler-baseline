#![no_std]
// TY-005: `Ty::Float(FloatKind)`. An `f32` parameter/return interns a
// `Ty::Float(F32)`. (`f64` works identically; `f16`/`f128` need feature
// gates and are out of scope for this row -- `f32`/`f64` cover it.)
pub fn f(x: f32) -> f32 {
    x
}
