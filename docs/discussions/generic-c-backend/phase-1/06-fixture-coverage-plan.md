# Phase 1 — codegen-coverage fixture plan (flipping the ledger to `pass`)

A **plan-first** doc (pre-task, per `.harness/generic-c/methodology.md`) for the next
generic-C Phase-1 workstream: authoring the codegen-coverage fixtures that flip the
**159 `covered-without-green`** matrix rows to green, driving the ledger verdict
`not-pass → pass`. The backend *lowering* is feature-complete (every construct lowers,
compiles, runs; the const exit gate is gap-free for reachable enum shapes after d2.3b);
what remains is **verification coverage** — committed fixtures that exercise each
construct end-to-end on the host.

## 0. Current ledger state (the gap)

`cargo run -p codegen-coverage-runner` → **verdict `not-pass`**, matrix rows **176** =
**169 covered + 7 out-of-scope**. Of the 169 covered, **10 already have a green fixture**
(the 11 committed diag-log fixtures) → **159 covered-without-green**. The 10 already-green:
`DL-1,2,3,4,6,7` + `TM-2,3,5,8` (diag-log + its `Call`/`SwitchInt`/`Return` terminators).
Gate `pass` ⇔ every `covered` row has ≥1 green fixture, no fixture FAILs, no drift
(`ledger.rs` verdict rule). **There are no `deferred` rows to lean on — all 169 must go
green.**

The 159, by family (exact IDs verbatim from `coverage_ledger.json`
`drift.covered_without_green` — these strings are what a `covers` entry must match):

| fam | n | rows |
|----|---|------|
| IT | 13 | IT-1,2,3,4,5,6,7,8,9,11,12,13,14 |
| IN | 11 | IN-1,2,3,4,5,6,7,8,9,10,11 |
| BD | 8 | BD-1,2,3,5,7,8,9,10 |
| TY | 18 | TY-1,2,3,4,5,6,7,8,9,10,11,13,14,15,16,17,18,19 |
| LY | 14 | LY-1..14 |
| CO | 14 | CO-1..14 |
| PL | 8 | PL-1..8 |
| OP | 3 | OP-1,2,3 |
| RV | 9 | RV-1..9 |
| UN | 3 | UN-1,2,3 |
| CK | 11 | CK-1..11 |
| PC | 4 | PC-1,2,3,4 |
| AG | 7 | AG-1..7 |
| BO | 8 | BO-1..5, BO-6..8, BO-9..11, BO-12..14, BO-15..18, BO-19..24, BO-25, BO-26 |
| ST | 4 | ST-1,2,3,4 |
| TM | 6 | TM-1,4,6,7,9,10 |
| AK | 8 | AK-1..8 |
| FS | 9 | FS-1,2,3,4,5,7,8,9,10 |
| DL | 1 | DL-5 |

> **`covers` matches verbatim, no range expansion** (`matrix.rs::is_row_id`,
> `ledger.rs::build`). `BO-1..5` is a single opaque ID; a `covers` entry naming a row
> absent from the matrix is a **hard `AbsentRowId` error** (aborts the run), and a green
> fixture covering a *non-`covered`* row is `green_without_covered` drift (`→ not-pass`).
> So every `covers` string below is an exact matrix ID, and only `covered` rows are declared.

## 1. The verification model

Author **`kind=rust` fixtures** (`crates/codegen-coverage-runner/fixtures/<name>/`:
`manifest.toml` + `input.rs` + `expected.stdout` [+ `expected.stderr` iff `expect=panic`]
+ `golden.c`). Each fixture **exercises a cluster of constructs and observes the computed
results** under a **dual oracle**: the byte-exact `golden.c` snapshot (`oracle.rs::snapshot`)
+ a behavioral oracle (`behavioral_stdout` for `expect=pass`, `behavioral_panic` for
`expect=panic`). The manifest declares the rows it covers:

```toml
name = "<name>"   kind = "rust"   entry = "regseq_main"
covers = ["BO-1..5", "UN-1", ...]   expect = "pass"   # or "panic" (+ expected.stderr)
```

`expect = "pass"` → clean exit, **stdout** compared. `expect = "panic"` → non-zero exit
(a signal death from `abort()` counts), **stderr** compared (`oracle.rs::behavioral_panic`).

**`covers` integrity convention (binding):** the harness verifies the fixture
compiles+runs+matches, NOT that a declared row was *genuinely* exercised — so a fixture
MUST only declare rows it actually drives, with an `input.rs` comment mapping each observed
line → the row(s) it proves. (Over-declaring would green a row no fixture exercises — the
exact failure the project's soundness discipline guards against.)

## 2. The observation oracle — how each construct is proven (Codex fix #4)

The diag-log `LogArg` carrier admits only **`u8..u64` / `i8..i64` / `bool` / `&str`**
(see any `diag_*/input.rs`). It does **not** carry `char`, `f16..f128`, `usize`/`isize`,
`u128`/`i128`, or pointers. And a memory *offset/size* is not a value you can print at all.
So the behavioral observation is **per-construct**:

| construct class | observation | mechanism |
|---|---|---|
| bool, `i8..i64`, `u8..u64` | **direct stdout** | `info!("{}", v)` |
| `char` | **cast→stdout** | `info!("{}", c as u32)` |
| `usize`/`isize` (incl. slice len, `PtrMetadata`) | **cast→stdout** | `info!("{}", n as u64 /* as i64 */)` |
| `f32`/`f64` | **bit-cast→stdout** | `info!("{}", x.to_bits() as u64)` (exact, width-faithful) |
| `f16`/`f128`, `u128`/`i128` | **boolean-checksum→stdout** | `info!("{}", (x == EXPECTED) as u8)` (carrier can't hold the width) |
| raw ptr / reference value | **relational / deref→stdout** | `info!("{}", (p == q) as u8)` or log the loaded pointee |
| **layout / offset / align / ZST / enum-shape** | **`cc` compiles the golden.c** | the backend already emits `_Static_assert(sizeof/_Alignof/offsetof == N)` per aggregate (`layout_emit.rs`, `enum_emit.rs`); the certified-size assertion is checked against the *emitted* struct by the host compiler — a wrong layout fails to compile. Proven by **building**, not stdout. |
| structural-only (Empty enum, `Never` return spelling) | **golden.c snapshot** | the emitted form appears in `golden.c` (snapshot oracle); `Never` adds an `expect=panic` run |
| panic / abort | **`expect=panic` + stderr** | `behavioral_panic`: non-zero exit + golden `expected.stderr` |

Every fixture *additionally* carries the always-on `golden.c` byte-exact snapshot.
**Layout correctness is a first-class win of the static-assert design:** a fixture need only
*contain* a struct/enum/union for its `_Static_assert` offsets to be enforced at `cc` time.

## 3. Master row→fixture accounting (one-to-one; Codex fixes #1, #2)

Every one of the 159 rows below maps to **exactly one** owning fixture (its primary prover),
or to the blocked / reclassify list (§4). A fixture MAY co-declare an envelope row, but each
row has a single primary here so completeness is mechanically checkable. Split into the
**envelope** (rows any `kind=rust` function fixture genuinely drives), the **batches**, and
the **deferred** set.

**Envelope — owned by B1 (the canonical foundational fixture; genuinely present in any
function body)** — function item + instance + CFG + signature frame, operands, plain assign:
`IT-1` (Function item), `IT-7` (NoMangle linkage — every fixture's `#[no_mangle] regseq_main`),
`IN-1` (emission_name), `IN-2` (Function w/ body), `IN-4` (fn_sig), `BD-1,2,3,5,7,8,9,10`
(blocks/locals/args/labels/entry), `FS-1,2,3` (sig envelope + Rust ABI), `OP-1,2,3`
(Copy/Move/Constant), `ST-1` (Assign), `RV-1` (Use), `RV-6` (BinaryOp), `RV-7` (UnaryOp).
*(Codex fix #2: this envelope is ONLY the function/body/sig frame — it does NOT silently
absorb statics, consts, ADTs, foreign fns, or item flags; those are owned by a fixture that
**contains** the construct, below.)*

| batch | rows owned (primary) | oracle |
|---|---|---|
| **B1** int-scalars | `TY-1,2,3,4` · `CO-1,2,3,4` · `BO-1..5, BO-6..8, BO-9..11, BO-12..14, BO-15..18, BO-19..24, BO-25` · `UN-1,2` *(+ the envelope above)* | stdout (char→u32; isize/usize→i64/u64; **i128/u128→boolean-checksum**) |
| **B2** floats + casts | `TY-5` · `CO-5` · `RV-5` · `CK-1,2,4,5,6,7,8,9,10,11` | stdout (f32/f64→`to_bits()`; f16/f128→boolean-checksum; ptr-casts→relational; int-casts→direct) |
| **B3** aggregates · places · refs · ptrs · slices | `TY-6,8,9,10,11,15,16,17,19` · `IT-4,6,11,12` · `IN-9,11` · `LY-1,2,3,4,6` · `AG-1,2,3,5,6,7` · `PL-1,2,3,4,5,6,7` · `RV-2,3,4,9` · `CO-6,7,8,10,11,12,13,14` · `UN-3` · `PC-4` · `ST-3,4` · `BO-26` | stdout (field/element/len/pointee values) **+ golden.c `_Static_assert`** for `LY-1,2,3,4,6` & ZST (`CO-6`, empty-tuple `TY-16`) |
| **B4** enums (runtime) | `TY-18` · `IT-5` · `IN-10` · `LY-5,7,8,9,10,11,12,13,14` · `PL-8` · `RV-8` · `ST-2` · `CO-9` · `AG-4` | stdout (discriminant/payload) + golden.c enum static-asserts; `LY-7` (Empty enum) = **structural golden.c** |
| **B5** control flow + calls | `TY-13` · `IN-5` · `IT-13` · `TM-1,7,9,10` · `PC-1,2` · `CK-3` | stdout (branch/loop/indirect-call results; drop side-effect logged) |
| **B6** panics / aborts *(10 fixtures — §4a)* | `AK-1`+`TM-6` · `AK-2` · `AK-3` · `AK-4` · `AK-5` · `AK-6` · `AK-7` · `AK-8` · `TM-4` · `TY-7` | `expect=panic` + stderr (`TY-7` also golden.c `void`/`_Noreturn` return) |
| **B7** fn signatures | `FS-4,5,7,8,9,10` | stdout (call results across Rust-ABI / `extern "C"` / variadic) |
| **B8** item-kinds *(stragglers that need containment)* | `IT-2,3,8,9,14` · `IN-3,7,8` | stdout (static/const values; libc `abs` for the foreign `IN-3`) + golden.c (`extern` decl, `section(...)`, opaque-handle fwd-decl, `const`-qualified global) |

**Per-family closure check** (every ID lands once): IT → B1(1,7)·B8(2,3,8,9,14)·B3(4,6,11,12)·
B4(5)·B5(13) = 13 ✓ · IN → env(1,2,4)·B8(3,7,8)·B5(5)·B3(9,11)·B4(10)·**blocked(6)** = 11 ✓ ·
BD → env(8) ✓ · TY → B1(1,2,3,4)·B2(5)·B3(6,8,9,10,11,15,16,17,19)·B6(7)·B5(13)·**blocked(14)**·
B4(18) = 18 ✓ · LY → B3(1,2,3,4,6)·B4(5,7,8,9,10,11,12,13,14) = 14 ✓ · CO → B1(1,2,3,4)·B2(5)·
B3(6,7,8,10,11,12,13,14)·B4(9) = 14 ✓ · PL → B3(1-7)·B4(8) = 8 ✓ · OP → env(3) ✓ · RV →
env(1,6,7)·B3(2,3,4,9)·B2(5)·B4(8) = 9 ✓ · UN → B1(1,2)·B3(3) = 3 ✓ · CK → B2(1,2,4-11)·B5(3) =
11 ✓ · PC → B5(1,2)·**blocked(3)**·B3(4) = 4 ✓ · AG → B3(1,2,3,5,6,7)·B4(4) = 7 ✓ · BO →
B1(7 IDs)·B3(26) = 8 ✓ · ST → env(1)·B4(2)·B3(3,4) = 4 ✓ · TM → B5(1,7,9,10)·B6(4,6) = 6 ✓ ·
AK → B6(8) ✓ · FS → env(1,2,3)·B7(4,5,7,8,9,10) = 9 ✓ · DL → **reclassify(5)** = 1 ✓.
**Total 159, each row exactly once.**

## 4. Deferred — NOT greened by this push (honest residual; Codex fix #5)

`pass` is reached when the §3 push (B1–B8) greens its **155** rows, **plus**:

### 4a. B6 is one fixture per abort path (Codex fix #3)

The host panic routine `abort()`s on the first panic and `expect=panic` checks a single
run's stderr — so each abort path is its **own** fixture: `b6_bounds` (AK-1 **+ TM-6**, the
`Assert` terminator), `b6_overflow_add` (AK-2), `b6_overflow_neg` (AK-3), `b6_div_zero`
(AK-4), `b6_rem_zero` (AK-5), `b6_misaligned` (AK-6), `b6_null_deref` (AK-7),
`b6_invalid_enum` (AK-8), `b6_unreachable` (TM-4), `b6_never` (TY-7, a diverging `-> !` fn).
→ **10 fixtures.** (TM-6 rides on `b6_bounds` because a `BoundsCheck` *is* an `Assert`
terminator; it needs no separate fixture.)

> **Constructibility — gated by the runner's bridge flags (a real harness dependency, not
> just an authoring risk).** The coverage runner bridges every fixture through
> `try_bridge_fixture_path` (`pipeline.rs:104`) — the **no-override** entry — and the bridge
> helper hardwires `overflow_checks = Some(false)` and `debug_assertions = Some(false)`
> (`test-ir-fixtures/src/lib.rs:427,431`). The manifest schema (`manifest.rs`) exposes **no**
> flag field. So which MIR asserts even *exist* in the bridged model splits the B6 roster:
>
> | rows | gating | status under the current runner |
> |---|---|---|
> | `AK-1` (bounds), `AK-4` (div0), `AK-5` (rem0), `TM-4` (unreachable), `TY-7` (`-> !`) | always-inserted / explicit divergence | **constructible now** — no flag needed |
> | `AK-2` (overflow), `AK-3` (overflow-neg) | `-C overflow-checks` | **needs plumbing**: `BuildOverrides.overflow_checks` already exists + is pre-authorized (`test-ir-fixtures/src/lib.rs:50`, F68-audit D1), but the runner doesn't thread it. Fix = a manifest field + `try_bridge_fixture_path_with_overrides` in the pipeline (small, already-authorized). |
> | `AK-6` (misaligned), `AK-7` (null-deref), `AK-8` (invalid-enum) | `-C debug-assertions` + MIR passes | **no override surface** — `BuildOverrides` is intentionally narrow (overflow-checks + a `-Z` allow-list only); adding `debug_assertions` is "a separate task with its own approval gate" (`test-ir-fixtures/src/lib.rs:46-48`). Worse, the production MIR-pass whitelist disables `CheckAlignment`/`CheckNull`/`CheckEnums` outright (`regseq-driver/src/callbacks.rs:19`), so these asserts are stripped at the pass level — the backend (which *does* carry their spec-pinned message catalog + lowering, `prelude.rs:65`, `body_emit.rs:1889`) never sees them. |
>
> So the §3 claim that `AK-1..5` are uniformly high-confidence was wrong — **`AK-2`/`AK-3`
> need the overflow-checks plumbing first.** `AK-6/7/8` need a decision (§5.4).

### 4b. Vtable-task-blocked — `{TY-14, PC-3, IN-6}`

These are `covered` (in P1 scope, spec-pinned) but fail loud in the backend today (the
unimplemented **vtable task**: dyn deref/reborrow at `body_emit.rs` ~`:984`/`:1256`, the
dyn fat-pointer carrier `mod.rs:1395`, `ClosureFnPointer`):

- **`TY-14`** `Dynamic` (dyn fat pointer) · **`PC-3`** `ClosureFnPointer` ·
  **`IN-6`** `virtual_idx` (the vtable slot index — `Some` only for `Virtual` instances,
  which arise *only* from dyn dispatch; unreachable without a trait object).

They go green only when the **vtable task** (a separate, drafted-later backend task) lands.
**They are NOT re-scoped to fake the gate** — marking in-scope rows out-of-scope to turn the
gate green is exactly the line the project's soundness discipline forbids. `pass` honestly
waits on the vtable task. *(Note: `UN-3` `PtrMetadata` and `PC-4` `Unsize` are NOT here —
their **slice** uses are greened by B3; only the **dyn** uses are vtable-gated, and a row
greens on any one fixture.)*

### 4c. `DL-5` — reclassify (matrix edit, not a fixture)

`DL-5` (`max_str_args_per_call = 8`) is a **bridge-side** D8 validation reject, not a backend
emission — not exercisable by a `kind=rust` codegen fixture (it's exercised by
bridge-coverage). The honest resolution is a **Doc-1 matrix edit**: `DL-5` status
`covered → out-of-scope` (reason: "bridge-side validation; covered by bridge-coverage, no
codegen emission"). This is legitimate under the matrix's own stability rule (a row that
moves out of a layer's scope → `out-of-scope`), and it removes `DL-5` from the gated set.

### 4d. `TY-7` / `LY-7` are coverable, not blocked (Codex correction)

Both were previously parked as "structural-only / blocked"; they are **coverable**:
`TY-7` (`Never`) has a `void`/`_Noreturn` return spelling reached by a diverging `-> !` fn →
the `b6_never` `expect=panic` fixture (golden.c shows the spelling, the run proves the
divergence). `LY-7` (`Empty` enum) emits **structurally** — the backend names every nominal
ADT instance (bridge discovery roots, methodology), so an empty enum behind a `*const`
field emits its form into `golden.c` → snapshot oracle. Both are owned in §3 (B6 / B4).

### The path to a genuine `pass`

B1–B8 push (155 rows green) **+** the **overflow-checks runner plumbing** (unblocks `AK-2`/
`AK-3`) **+** the **`AK-6/7/8` disposition** (§5.4) **+** vtable task (`TY-14, PC-3, IN-6`
green) **+** `DL-5` reclassify (leaves the covered set) **→** every `covered` row green, no
drift, no FAIL → `pass`. The residual is fully accounted, never bookkeeping — and the panic
rows now carry their **concrete harness dependency** rather than an authoring hand-wave.

## 5. Fixture roster + decisions to settle (architect)

> **Settled (2026-05-29, architect):** **1.** broad granularity (~17 fixtures). **2.**
> multi-agent **workflow** vehicle (independent batches, one agent each + a final
> ledger-reconciliation pass). **4.** `AK-6/7/8` → **reclassify out-of-scope** (option b),
> alongside `DL-5`. (Decision 3 sequencing stands as written.)

**Roster: ~17 fixtures** — B1, B2, B3, B4, B5, B7, B8 (7 broad) + B6 ×10 (one per abort
path). B3 is the largest (50 rows); B1 (envelope + scalars) and B4 (enums) are next. Split
candidates if a single `input.rs` proves unwieldy: **B3 → B3a** (structs/tuples/arrays/unions
+ layout) **/ B3b** (refs/raw-ptrs/slices/`PtrMetadata`/`Unsize`/`Offset`); **B4 → B4a**
fieldless-`Direct` **/ B4b** payload+niche; **B1 → B1a** arith/overflow **/ B1b**
bitwise/shift/compare. Splitting only subdivides a `covers` set — never changes the §3
accounting.

1. **Granularity** — broad (fastest to `pass`, ~17 files) vs. one-per-sub-family (clearer
   localization, ~40 files). **Recommend broad**, splitting B3 first if needed.
2. **Authoring vehicle** — file the batches as linear `task.py` tasks, or **fan them out via
   a multi-agent workflow** (the batches are independent — one agent per batch authors +
   compiles/runs/blesses + verifies `covers`; a final ledger-reconciliation pass). A genuine
   workflow fit (unlike the sequential d2.3b). B6's 10 panic fixtures are independent leaves.
3. **Sequencing of the residual** — the B1–B8 push and the vtable task are independent; run
   the push first (the high-confidence win, coverage 10 → ~162), draft the vtable task in
   parallel, do the `DL-5` reclassify last (a one-line matrix edit) so the gate flips in one
   reconciled step.
4. **`AK-6/7/8` (misaligned / null-deref / invalid-enum) disposition** — these need
   `-C debug-assertions`, which the runner has no override for and which the F68 audit
   deliberately left off the authorized fixture-compile surface (§4a). Two honest paths:
   **(a)** a separate **approval-gated harness task** that adds a `debug_assertions`
   override to `BuildOverrides` + the runner manifest, then authors the 3 fixtures (most
   complete — honors "covered ⇒ green"; but opens a flag the audit gated, and even then the
   misalign/null/invalid-enum MIR asserts must be shown constructible from `kind=rust`); or
   **(b)** **reclassify `AK-6/7/8` out-of-scope-for-codegen** (matrix edit, like `DL-5`),
   reason: *not constructible from `kind=rust` fixtures under the F68-authorized flag surface
   (debug-assertions off by design); the backend's message catalog for them is spec-pinned
   but unreachable in the bridged model.* **Recommend (b)** unless the architect wants the
   debug-assertions surface for other reasons — it keeps the audit's flag gate intact and
   reaches `pass` without an approval round, and the rows stay traceable (out-of-scope ≠
   deleted). `AK-2/AK-3` are NOT part of this fork — their `overflow_checks` override is
   already authorized, so they are plumbed-and-greened, not reclassified.

## 6. Recommended order

B1 (envelope + most scalars) → B3 (the big structural block; split if needed) → B2 → B4 →
B5 → B7 → B8 → **B6**. B6's prerequisites, settled before its fixtures: decide the §5.4
`AK-6/7/8` fork (recommend reclassify), and plumb the `overflow_checks` manifest override
into the runner so `AK-2`/`AK-3` are constructible; then author the constructible panic
fixtures (`AK-1,2,3,4,5` + `TM-4` + `TY-7`, with `TM-6` on `b6_bounds`).
After each batch: re-run `cargo run -p codegen-coverage-runner`, confirm the declared rows
flip green and the ledger stays byte-stable ([[codegen-ledger-determinism]],
[[golden-c-emit-process-determinism]] — corpus snapshot tests run isolated). Then: vtable
task (`→ TY-14, PC-3, IN-6`); `DL-5` (+ optionally `AK-6/7/8`) matrix reclassify →
**genuine `pass`**.

## 7. Grounding anchors

- Matrix: `phase-1/01-construct-surface-and-matrix.md` (the 7 out-of-scope rows —
  `BD-4, BD-6, FS-6, FS-11, IN-5b, IT-10, TY-12` — are already excluded from the 159).
- Ledger: `.harness/generic-c/coverage_ledger.{json,md}` (`drift.covered_without_green` =
  the exact 159; `ledger.rs` verdict rule). Harness: `crates/codegen-coverage-runner/src/{manifest.rs
  (Expect::{Pass,Panic}), matrix.rs (verbatim row IDs), oracle.rs (snapshot/stdout/panic),
  pipeline.rs, ledger.rs}`. Static-assert emission: `crates/regseq-backends/src/generic_c/{layout_emit.rs,
  enum_emit.rs}`. Existing fixtures: `fixtures/diag_*/` (the `covers` + dual-oracle pattern).
- Verification flow: `phase-1/03-verification-flow.md`. Determinism: emit byte-stable per
  clean process; corpus snapshot isolated.
