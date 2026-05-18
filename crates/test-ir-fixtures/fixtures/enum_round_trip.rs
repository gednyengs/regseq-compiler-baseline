// Enum-layout round-trip fixture for regseq-driver-54.
//
// Declares one enum of each `EnumShape` arm the bridge must
// construct (`Empty`, `Single { scalar: None }`,
// `Single { scalar: Some(_) }`, `Multiple { encoding: Direct }`,
// `Multiple { encoding: Niche }`) plus straight-line call-sites
// that produce both `AggregateKind::Enum` rvalues (direct
// constructors via re-assignment per the
// `bridge_body_remaining_rows` pattern at
// `crates/regseq-driver/tests/bridge_body.rs:406-410`) and the
// `SetDiscriminant` MIR statement (rustc lowers `let mut e =
// VariantA; e = VariantB(...);` into `SetDiscriminant` writes plus
// per-field stores).  Function bodies are kept straight-line so
// the bridge's joint-validation DFS-RPO pre-check (`visit.rs::BRpo`)
// passes without requiring a reorder pass on this fixture.
//
// Per regseq-driver-54 Test Plan item C the integration test for
// this fixture asserts BOTH `model.validate(...)` AND
// `PrettyPrinter::new(&model).render()` complete without error --
// validate covers the `validate_layouts` enforcement of
// `LayoutKind::Enum`, render covers the four pretty.rs panic sites
// that only fire during emission (see spec section 11.1 for the
// shape contract).

#![no_std]

pub enum Empty {}

pub enum SingleOrdinary {
    Only,
}

#[repr(u8)]
pub enum SingleReprInt {
    A = 5,
}

#[repr(u8)]
pub enum MultiDirect {
    A = 1,
    B = 2,
    C = 3,
}

pub enum MultiNiche {
    None,
    Some(&'static u32),
}

pub fn make_single_ordinary() -> SingleOrdinary {
    SingleOrdinary::Only
}

pub fn make_single_repr_int() -> SingleReprInt {
    SingleReprInt::A
}

pub fn make_multi_direct_a() -> MultiDirect {
    MultiDirect::A
}

pub fn make_multi_direct_b() -> MultiDirect {
    MultiDirect::B
}

pub fn make_multi_direct_c() -> MultiDirect {
    MultiDirect::C
}

pub fn make_multi_niche_some(ptr: &'static u32) -> MultiNiche {
    MultiNiche::Some(ptr)
}

pub fn make_multi_niche_none() -> MultiNiche {
    MultiNiche::None
}

// Re-assignment idiom that produces `SetDiscriminant` writes for
// direct-tag multi-variant enums in MIR (mirrors the
// `bridge_body_remaining_rows::set_row` shape at
// `crates/regseq-driver/tests/bridge_body.rs:406-410`). The
// resulting MIR carries `AggregateKind::Enum` constructors and at
// least one `SetDiscriminant` statement, exercising the body-
// invariant chain against a direct-encoded multi-variant enum.
pub fn set_discriminant_direct() -> MultiDirect {
    let mut e = MultiDirect::A;
    e = MultiDirect::B;
    e
}

// Same re-assignment idiom for the niche-encoded multi-variant
// enum. The MIR for the second assignment writes the niche-bearing
// payload directly for the untagged variant; the variant index
// flips via either a `SetDiscriminant` (for the niched `None`
// arm) or a payload-store-only path for the untagged arm,
// depending on the variant ordering. Either way the bridged enum
// layout survives `model.validate` and `render`.
pub fn set_discriminant_niche(ptr: &'static u32) -> MultiNiche {
    let mut e = MultiNiche::None;
    e = MultiNiche::Some(ptr);
    e
}
