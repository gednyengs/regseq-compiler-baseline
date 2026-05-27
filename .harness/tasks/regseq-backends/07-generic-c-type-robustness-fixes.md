# regseq-backends-07: Generic-C type-emission robustness fixes (c1 soundness follow-up)

## Description

Fix the **4 edge-case defects** a Codex soundness review found in the task-c1
type-system implementation (`regseq-backends-06`, `f410115`). c1's common
path is correct + fully tested (34 green tests; bridged `repr(Rust)` / union
/ tuple / array aggregates compile with the §14 `_Static_assert` guards
holding), but the review surfaced holes **beyond the tested scope**:

1. **(#2, HIGH — backend panic.)** `Node::Slice` contributes **no** edge in
   `node_edges` (`generic_c/mod.rs:134`), so a slice whose element is a
   **structural aggregate reachable only through the slice** (e.g.
   `&[(u32, u32)]`, `&[[Inner; 4]]`) never enters the `NameTable` — and
   emitting the slice struct's `data` field then calls
   `NameTable::name(Node::Agg(elem))`, which **panics** (`mod.rs:72-76`).
2. **(#4, MED — silent wrong qualifier.)** `ptr_to` prepends `const` to the
   **fully-rendered inner declaration** (`generic_c/types.rs:163-168`); for a
   pointee that is itself a pointer / fn-pointer, this qualifies the wrong C
   type (e.g. `&*const T`, `&&T`). It compiles but is the **wrong const**;
   the `_Static_assert` layout guards do **not** catch a qualifier error.
3. **(#1, low — name collision.)** Synthetic `_padN` filler members
   (`generic_c/layout_emit.rs:167, 184`) can collide with a real field
   literally named `_pad0` (a legal sanitized field name) → duplicate-member
   compile error.
4. **(#3, low — invalid C.)** `fn_ptr_decl` renders the return type as a bare
   `c_type` string then appends `(*decl)(params)` (`generic_c/types.rs:208-220`);
   a **nested fn-pointer return** (`fn() -> fn() -> i32`) emits the invalid
   `int (*)(void) (*cb)(void)` instead of recursing the declarator (gc §15
   "nested `FnPtr` declarators recurse").

None block the critical path to the first green fixture (task e). This task
hardens c1's type emission so slices-of-aggregates, nested pointers, and the
declarator edge cases are correct before later tasks rely on them.

## Input Context

### Source of the findings
- The c1 soundness review (Codex, recorded in `.harness/HANDOFF.md` § Current
  state, c1-close entry). Each fix below carries the review's diagnosis +
  the verified code anchor.

### Spec anchors (`docs/discussions/regseq-model/generic-c-backend.md`)
- **§3.2** (lines 116–145): the `rust_slice_<T>` shape (`data` is a pointer
  to the element; the element's *name* must exist, but a forward-decl
  suffices since `data` is a pointer — **except** an array element, see §15).
- **§15** (lines 988–1044): the by-value containment **ordering** rule +
  the **fat-pointer exception** (now pinned in §15 as part of this work): a
  containing aggregate edges to the lowered fat struct; the fat struct's
  `data` pointer needs only a forward-decl of the element **except** for a
  slice whose element is a `Ty::Array(base, N)` (`data` is `base (*)[N]`, and
  a C array needs its element complete even behind a pointer → an *ordering*
  edge to `base`). Also "nested `FnPtr` declarators recurse through the same
  table". Key distinction this task formalizes: **discovery** (a type must be
  *named + forward-declared*) is broader than **by-value ordering** (a type
  must be *complete before* its container) — and §15 now carries the
  array-element-slice ordering case (no doc contradiction).
- **§3.1** (lines 104–115): the `const`/mutability rules the declarator must
  place correctly.

### Code anchors (`crates/regseq-backends/src/generic_c/`, at `f410115`)
- `mod.rs:36-57` — `Node::{Agg, Slice, TraitObj}`.
- `mod.rs:132-141` — `node_edges` (the single edge function used for **both**
  discovery and topo-sort; this task splits the two — see Fix A).
- `mod.rs:190-219` — `containment_target` / `unsized_pointee_node`.
- `mod.rs:228-252` — `discover` / `dfs_discover`.
- `mod.rs:266-294` — `topo_sort` / `topo_visit`.
- `types.rs:150-169` — `ptr_to` (the `const`-prefix bug, Fix B).
- `types.rs:202-221` — `fn_ptr_decl` (the nested-return bug, Fix D).
- `layout_emit.rs:163-187` — the `_padN` filler emission (Fix C).

### Prior task
- `regseq-backends-06` (`f410115`, done): the c1 implementation these fixes
  patch. The 34 existing tests must stay green.

## Implementation Notes

### Fix A (#2): split discovery edges from by-value ordering edges

The root cause: `node_edges` serves **both** `discover` (what must be
named + forward-declared) and `topo_sort` (what must be *complete before*
this node). Those are different: a slice's `data` pointer needs the element
**named + forward-declared** (a discovery edge) but **not** complete (no
ordering edge — a pointer to a forward-declared type is legal C).

**The C completeness rule that drives the split.** A pointer *to a struct/
union* needs only a forward-decl (`struct S *p;` with `struct S;` is legal).
But a C **array type needs its element complete even behind a pointer** —
`struct Inner; struct S { const struct Inner (*data)[4]; };` is rejected by
`cc` ("array type has incomplete element type"). So the slice `data`
spelling decides what the element needs:
- slice element is a **struct/union/tuple/closure** → `data` is `T *` →
  element needs only **forward-decl** (a *discovery* edge);
- slice element is an **`Ty::Array([base; N])`** → `data` is `base (*)[N]` →
  `base` (recursively, through nested arrays) needs to be **complete** (an
  *ordering* edge).

Split `node_edges` into two functions:
- **`discovery_edges(node)`** — used by `discover`/`dfs_discover`: the
  current set **plus**, for `Node::Slice { elem, .. }`, an edge to the
  element's aggregate node (resolve `elem` through `Ty::Array` to its base;
  aggregate base → `Node::Agg(base)`; unsized-ref base → its fat node). This
  makes the element **named + bank-2 forward-declared** (fixes the panic for
  every aggregate element). `Node::TraitObj` keeps **no** edge (`data` is
  `void *`; the `<Trait>_VTable` is forward-declared, deferred).
- **`ordering_edges(node)`** — used by `topo_sort`/`topo_visit`: the current
  by-value set **plus**, for `Node::Slice { elem, .. }` whose `elem` is a
  `Ty::Array` (recursively), an ordering edge to the array **base**
  aggregate (it must be complete before the slice). For a slice with a
  **non-array** aggregate element, **no** ordering edge — the `data` pointer
  only needs a forward-decl, and omitting it is what keeps legal self-
  reference like `struct Inner { xs: &[Inner] }` from being a false
  `ContainmentCycle` (Inner → Slice{Inner} is a *pointer* edge, not by-value).

After this: a slice-only-reachable element is discovered + forward-declared,
so the `data` field spells it without panic; a struct/tuple element needs no
ordering (forward-decl suffices); an array element is ordered complete-first.
**Known limitation (document, don't over-engineer):** a genuinely cyclic
array-element slice (`struct Inner { xs: &[[Inner; 4]] }`) is **not C-
representable** with this spelling (Inner needs the slice complete as a
by-value field, the slice needs Inner complete as an array element) → it
surfaces as an emission error. That shape is exotic; surfacing it as an error
(not a panic, not silent mis-emission) is acceptable for c1 — note it rather
than solving it.

### Fix B (#4): place `const` within the declarator, not as a prefix

`ptr_to` must qualify the **pointee type**, not prefix `const` to the
rendered inner string. Thread the qualifier into the declarator so each
pointer level places `const` adjacent to the type it qualifies. Required
outputs (add as tests):
- `&u32` → `const uint32_t *<decl>` (pointer to const u32).
- `&mut u32` / `*mut u32` → `uint32_t *<decl>`.
- `*const u32` → `const uint32_t *<decl>`.
- a nested case, e.g. `& &u32` (shared ref to shared ref) → the
  pointer-to-(pointer-to-const-u32) with **valid, correctly-placed**
  qualifiers (no `const const`, no qualifier on the wrong level). Pin the
  exact expected spelling in the test from a C-correct reading; the
  implementer confirms it compiles + round-trips the intended constness.

Keep the §3.1 `ref_is_const` / `contains_unsafe_cell` logic unchanged — only
the **placement** of the resulting qualifier is wrong today.

### Fix C (#1): collision-proof pad-filler names

Generate **every** synthetic padding member's name from a set that **cannot
collide** with the aggregate's real (sanitized) field names — this covers
**both** the struct inter-field/trailing `_padN` fillers (`layout_emit.rs:167,
184`) **and** the union trailing `_pad` filler (`layout_emit.rs:210-212`,
which today is a plain `_pad` and can collide with a real union field named
`_pad`). Either (a) compute the emitted field-name set first and pick filler
names avoiding it, or (b) use a reserved prefix the sanitizer can never
produce from a Rust identifier (document the invariant). Add tests with a
struct field named `_pad0` (+ a gap) **and** a union field named `_pad` →
distinct, non-colliding members.

### Fix D (#3): recursive fn-pointer-return declarator

`fn_ptr_decl` must build the return type via the **recursive declarator**
(the same technique `ptr_to` uses for pointer-to-array/fn-ptr), placing the
outer `(*decl)(params)` **inside** the return type's declarator when the
return is itself a function pointer or array. Add a test for
`fn() -> fn() -> i32` (and a fn returning a pointer-to-array) → valid C that
compiles.

## Non-Goals
- No new constructs (enum bodies = c2; `dyn`/vtable = later; bank 4/5 =
  task d). This task only **hardens** c1's existing type emission.
- No change to the §14 offset/padding math, the discovery **roots** (still
  nominal instances — task d widens them), the determinism model, or the
  naming derivation — only the 4 fixes above.
- No green codegen-coverage fixture / no Doc-1 status flip; ledger verdict
  stays `not-pass`.
- No deps; no harness changes (reuse `toolchain` from tests).

## Test Plan
### B1: regression — the 34 existing tests stay green
`cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`.

### B2: the 4 fixes (new tests)
- **A**: (i) `&[(u32, u32)]` and `&[Inner]` (element reachable **only** via
  the slice) → `emit_generic_c` returns `Ok` (no panic), the element is
  bank-2 forward-declared, the TU **compiles** (reuse
  `toolchain::compile_and_run`), and `struct Inner { xs: &[Inner] }` does
  **not** raise `ContainmentCycle` (non-array element → discovery edge only).
  (ii) `&[[Inner; 4]]` (array element, non-cyclic) → `Inner` is ordered
  **before** the slice struct and the TU compiles (the array-element ordering
  edge). (iii) the exotic cyclic `struct Inner { xs: &[[Inner; 4]] }` → a
  clean emission **error** (not a panic, not silent) — assert the error path.
- **B**: declarator unit tests for `&u32` / `&mut u32` / `*const u32` /
  the nested case → exact expected C; a compile test that the nested form is
  valid C.
- **C**: a struct with a real `_pad0` field + a gap, **and** a union with a
  real `_pad` field → distinct, non-colliding members; both compile.
- **D**: `fn() -> fn() -> i32` declarator → valid C, compiles.

### B3: determinism + ledger + scope
`emit_generic_c` byte-stable; ledger unchanged (`not-pass`); diff confined to
`crates/regseq-backends/src/generic_c/{mod,types,layout_emit}.rs` + tests.

### B4: fmt + clippy
`cargo fmt --all -- --check` + `cargo clippy -p regseq-backends -p
codegen-coverage-runner --all-targets -- -D warnings` clean.

## Review
### Files to send to Codex
- This spec; `generic_c/{mod,types,layout_emit}.rs` (at HEAD); gc §3.1/§3.2/§15;
  `regseq-model/src/{layout,ty}.rs`.

### Reviewer focus
- **Fix A**: is the discovery/ordering edge split correct — does it discover
  + forward-declare the slice element **without** adding a by-value ordering
  edge (which would false-cycle `struct Inner { xs: &[Inner] }`)? Is
  `TraitObj` correctly excluded?
- **Fix B**: do the nested-pointer `const` placements emit valid + correctly-
  qualified C for `&u32` / `&mut u32` / `*const u32` / `&&u32`?
- **Fix C/D**: collision-proof pad names; recursive fn-ptr-return declarator.
- Regression risk: the 34 c1 tests + the common path stay correct;
  determinism preserved.

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/07-generic-c-type-robustness-fixes.md —
4 robustness fixes to the c1 generic-C type emission (slice-element discovery
edge / panic #2; declarator const-placement #4; pad-name collision #1; nested
fn-ptr-return declarator #3). Verify each proposed fix is correct and
in-scope, especially Fix A's split of DISCOVERY edges (name + forward-decl)
from by-value ORDERING edges (complete-before) — confirm it discovers the
slice element without adding a by-value edge that would false-cycle
`struct Inner { xs: &[Inner] }`. Check Fix B's const placements are C-correct
for nested pointers. Confirm the fixes don't regress c1's tested common path
or determinism. Read generic_c/{mod,types,layout_emit}.rs at HEAD + gc
§3.1/§3.2/§15.
</task>
<grounding_rules>
- Cite file:line. Mark hypotheses. Don't speculate about unread code.
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
FIX-A (discovery/ordering split): PASS / FAIL with citation
FIX-B (const placement): PASS / FAIL with citation
FIX-C (pad names) / FIX-D (fn-ptr return): PASS / FAIL
REGRESSION/DETERMINISM RISK: PASS / FAIL
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>
<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `cargo fmt --all -- --check` + `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`
  green (the 34 existing + the new fix tests).
- `emit_generic_c` byte-stable; ledger verdict unchanged (`not-pass`).
- `git diff --stat` confined to `generic_c/{mod,types,layout_emit}.rs` + tests
  + registration.
- Codex Task Review: APPROVED or APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria
1. **#2 fixed**: `emit_generic_c` does **not** panic on a slice whose element
   aggregate is reachable only via the slice. Discovery and by-value ordering
   use **separate** edge sets: a struct/tuple element gets a discovery edge
   (forward-decl suffices) so `&[(u32,u32)]` / `&[Inner]` compile and `struct
   Inner { xs: &[Inner] }` does **not** raise `ContainmentCycle`; an **array**
   element (`&[[Inner;4]]`) additionally gets an ordering edge (complete
   before the slice) so it compiles; the exotic cyclic
   `struct Inner { xs: &[[Inner;4]] }` surfaces as a clean emission error
   (documented limitation), not a panic.
2. **#4 fixed**: pointer/ref `const` is placed within the declarator;
   `&u32`→`const uint32_t *`, `&mut u32`→`uint32_t *`, `*const u32`→`const
   uint32_t *`, and the nested case is valid + correctly qualified.
3. **#1 fixed**: **every** synthetic padding member (struct `_padN` + union
   `_pad`) never collides with a real field name (tests with a `_pad0` struct
   field and a `_pad` union field).
4. **#3 fixed**: nested fn-pointer return (`fn()->fn()->i32`) emits valid C.
5. **No regression**: the 34 c1 tests stay green; determinism preserved;
   ledger unchanged; diff confined to the Quality-Gate set.
