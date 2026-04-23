# User-Data Initialization: Codex Adversarial Debate

Three-round Codex adversarial review transcripts supporting the
design in `../user-data-initialization.md`. Preserved in-repo so
that future sessions can audit the design provenance without
depending on ephemeral `/tmp` files.

## Round summaries

### Round 1 -- `round-1-initial-debate.md`

Evaluated six initialization-rule options against Regseq's goals
(minimize semantic tax, map cleanly to `kzalloc`, stay additive,
defensible). Recommended **Option III**
(`Option<T: Copy>` OR `T: Zeroable + Copy`) over **Option II**
(`Default + Copy`). Discovered Option II is toolchain-blocked:
pinned nightly 2026-02-01 requires unstable `const_default` and
`const_trait_impl` features to evaluate `T::default()` at compile
time, and no existing machinery evaluates trait-method calls at
compile time. Also confirmed `Option::unwrap`/`expect` lower to
no-ops in the semantic walker and `core::panic` isn't classified
at all, so forcing `.unwrap()`/`.expect()` for always-seeded state
is unsafe.

### Round 2 -- `round-2-revised-tier2.md`

Reviewed the revised `ZeroInit` safe-trait-with-unsafe-escape
design. Verdict: **revise-and-re-review**. Key finding: the design
is illegal Rust. Verified locally that rustc rejects
`unsafe impl` of a safe trait (E0199) and safe `impl` of an unsafe
trait (E0200). Can't have both. Plus identified 10-12 compiler
touch-points beyond the initial 5, deferred Tier-2/Tier-3
implementation until motivating fixtures surface, and recommended
`unsafe trait ZeroInit {}` with derive-generated `unsafe impl` as
the legal form if the trait-based design is pursued later.

### Round 3 -- `round-3-final-design.md`

Reviewed the collapsed two-rule design (compiler-controlled
whitelist + `Option<T: Copy>`; no trait, no derive, no author
opt-in). Verdict: **adopt-with-amendments**;
**CONVERGENCE: converged**. Confirmed:

- Struct recursion via `field.ty(tcx, args)` is sound.
- Foreign-struct acceptance requires no namespace restriction.
- Outer `Copy` check must be preserved (blocks `UnsafeCell<u32>`).
- Cycle guard must key by monomorphized type, not `DefId`
  (counterexample: `Boxed<u32>` vs `Boxed<NonZeroU32>` would share
  a `DefId`-keyed visited entry).
- Primitive whitelist must be exact; do not pass through `char`,
  `u128`, `i128` currently present in
  `normalize/surface.rs:1102`.
- Backend realizability is the practical gate: Linux's scalar-to-C
  mapper at `linux/plan.rs:400` supports only
  `u8, u16, u32, u64, bool` today. This led to the **Path A**
  scope decision: limit the Stage-1 Rule-1 primitives to this
  subset and widen pull-based when fixtures motivate it.

## How to add new transcripts

If this design gets re-opened in a future round (e.g., when
implementing Tier 2 author opt-in or Tier 3 niche-init
hardening):

1. Save each Codex round output as a new file in this directory,
   named `round-N-<description>.md`.
2. Update this README's "Round summaries" section.
3. Reference the new round from `../user-data-initialization.md`
   so the design doc's provenance stays auditable.

Do NOT reference ephemeral `/tmp/` paths from committed docs --
future sessions will not have those files.
