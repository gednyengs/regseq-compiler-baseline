// Instance-construction completeness fixture for
// regseq-driver-55. Exercises every in-scope source kind exactly
// once plus the synth-VTableType path so the Phase-5
// Instance-construction passes (Changes C/D/E) and the
// Item-vs-Instance coverage validator (Change G) are locked
// end-to-end.
//
// Per regseq-driver-55 Test Plan item C this fixture is driven by
// `crates/regseq-model/tests/e2e_instance_completeness.rs`, which
// asserts the Instance shapes and pins the rendered form.
//
// Authoring constraints (matching the established `enum_round_trip`
// + `trait_object` fixtures):
//   * Function bodies are kept straight-line: the bridge's
//     joint-validation DFS-RPO pre-check rejects non-RPO-ordered
//     basic blocks and the bridge has no block-reorder pass, so a
//     `match` discriminator is replaced by a `&Enum`-in-signature
//     touch that still forces Phase-2 `Ty::Enum` discovery.
//   * `make_and_dispatch` constructs a concrete `Reader` and
//     coerces `&Point` to `&dyn Reader` -- the `Cast::Unsize` site
//     that synthesises the VTableInstance the virtual call (and
//     hence the synth-`VTableType` `Ty::Struct` view Change E
//     covers) requires.
// The fixture is `#![no_std]` and free of `regseq::*` macros so
// the bridge sees it as plain Rust source.

#![no_std]

pub static COUNTER: u32 = 0;
pub static mut FLAG: u32 = 0;
pub const LIMIT: u32 = 64;

pub struct Point {
    x: i32,
    y: i32,
}
pub enum State {
    Idle,
    Active(u32),
    Error,
}
pub union Pair {
    pub x: u32,
    pub y: i32,
}
pub struct Wrap<T> {
    v: T,
}

pub fn touch_point(p: &Point) -> i32 {
    p.x
}
pub fn touch_pair(p: &Pair) -> u32 {
    // Safe field-access on a `union` shape forces Phase-2
    // discovery of `Ty::Union` for `Pair`. Without this touch,
    // the union has no reachable use-site and no
    // `Ty::Union { item: Pair, .. }` entry is interned, so
    // Change E's `model.types` walk would not see it.
    unsafe { p.x }
}
pub fn touch_state(_s: &State) -> u32 {
    // `&State` in the signature forces Phase-2 `Ty::Enum`
    // discovery; the body is straight-line so the DFS-RPO
    // pre-check passes (a `match` here would not).
    0
}
pub fn use_u32(w: &Wrap<u32>) -> u32 {
    w.v
}
pub fn use_u64(w: &Wrap<u64>) -> u64 {
    w.v
}

pub trait Reader {
    fn read(&self) -> u32;
}
impl Reader for Point {
    fn read(&self) -> u32 {
        self.x as u32
    }
}
pub fn dispatch(r: &dyn Reader) -> u32 {
    r.read()
}
pub fn make_and_dispatch() -> u32 {
    let p = Point { x: 1, y: 2 };
    dispatch(&p)
}
