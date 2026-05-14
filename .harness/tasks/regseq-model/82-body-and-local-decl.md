# regseq-model-82: Body + LocalDecl + LocalId + push_synthetic_local + LocalDecls allocation API + joint validation walk hooks (P1a-DS-14b)

## Description

Implement the `Body` IR data shape per `spec.md` §25: the `Body`
struct (`{ basic_blocks, local_decls, arg_count, span }`), the
`LocalDecl` struct (`{ ty, mutability, span, debug_name }`), the
`LocalId` typed-index (with `RETURN_PLACE = 0` const-body macro
arm consumption), the LocalDecls allocation API (`Body::push_local`
+ `Body::push_synthetic_local`), and the joint validation walk
hooks (the `Visitor` trait scaffolding per §25.6) consumed by
P1a-VAL-01 / P1a-VAL-02 / P1a-VAL-03.

`Body` is the function-level container that binds the basic-block
table and per-local declarations into a unit of MIR. Per Option B
(stage-12 D-12-1.4), `Body` lives **structurally inside Function
Instances** on `InstanceKind::Function { body: Option<Body>, ... }`
per §9.4 -- there is **no separate `model.bodies` table**, **no
`BodyId`**, and **no body-level `fn_sig` field** (the signature
lives Instance-side).

This task is the convergence point of the body-level walks
deferred from stages 5-10 (Place / Operand / Rvalue / Statement /
Terminator / BasicBlock site invariants). The walk-implementation
itself is sited in P1a-VAL-01 / P1a-VAL-02 / P1a-VAL-03; this task
provides the visitor-trait surface they consume.

## Input Context

- `docs/discussions/regseq-model/spec.md` §25.1 (`Body` struct +
  layout convention + dropped fields; `spec.md:11102-11184`)
- `docs/discussions/regseq-model/spec.md` §25.2 (`LocalDecl`
  struct + named-local lift + dropped fields;
  `spec.md:11185-11236`)
- `docs/discussions/regseq-model/spec.md` §25.3 (`LocalId`
  typed-index + sigil + size assertions; `spec.md:11237-11276`)
- `docs/discussions/regseq-model/spec.md` §25.4 (`FnSig`
  location -- Instance-side per Option B; `spec.md:11277-11333`)
- `docs/discussions/regseq-model/spec.md` §25.5 (Body-vs-Item
  linkage; `spec.md:11334-11427`)
- `docs/discussions/regseq-model/spec.md` §25.6 (Joint validation
  walk + Visitor trait; `spec.md:11428-11568`)
- `docs/discussions/regseq-model/spec.md` §25.8 (LocalDecls
  allocation API; `spec.md:12147-12198`)
- `docs/discussions/regseq-model/spec.md` §25.9 (Per-stage
  invariants registry; `spec.md:12199-12251`)
- `crates/regseq-model/src/basic_block.rs` (`BasicBlock`,
  `BlockId` from regseq-model-81)
- `crates/regseq-model/src/statement.rs` (`Statement` from
  regseq-model-79)
- `crates/regseq-model/src/terminator.rs` (`Terminator`,
  `CallCallee` from regseq-model-80)
- `crates/regseq-model/src/place.rs` (`Place` from regseq-model-76)
- `crates/regseq-model/src/operand.rs` (`Operand` from
  regseq-model-77)
- `crates/regseq-model/src/rvalue.rs` (`Rvalue` from
  regseq-model-78)
- `crates/regseq-model/src/idx.rs` (the `newtype_index!`
  const-body macro arm extended in regseq-model-81)
- `crates/regseq-model/src/source.rs` (`Span`)
- `crates/regseq-model/src/ids.rs` (`TyId`, `StringId`,
  `Mutability`)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  P1a-DS-14b row)

## Implementation Notes

Create new modules:
- `crates/regseq-model/src/body.rs` -- the `Body` struct, the
  `LocalDecl` struct, and the LocalDecls allocation API.
- `crates/regseq-model/src/visit.rs` -- the `Visitor` trait
  with default `super_X` recursions per §25.6.1.

`LocalId` introduction lives in `crates/regseq-model/src/idx.rs`
(the existing `newtype_index!` macro now supports const-body
arms per regseq-model-81). Re-export `LocalId`, `Body`,
`LocalDecl`, and the `Visitor` trait from `lib.rs`.

### Public API -- LocalId typed-index

Per `spec.md:11240-11245`:

```rust
newtype_index! {
    pub struct LocalId {
        const RETURN_PLACE = 0;
    }
}
```

Located in `crates/regseq-model/src/idx.rs` alongside `BlockId`.
Per `spec.md:11261-11269`, append size + niche assertions to
`crates/regseq-model/src/lib.rs`:

```rust
const _: () = assert!(core::mem::size_of::<LocalId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<LocalId>>() == 4);
```

Sigil convention per `spec.md:11252-11255`: textual rendering
`LocalId(N) -> _N` (e.g., `_0` for `RETURN_PLACE`, `_1` for
first argument, `_<arg_count+1..>` for temporaries). Rendering
site is P1a-PP-03; document the sigil as a forward-pointer in
the type-level doc comment.

### Public API -- Body struct

Per `spec.md:11104-11111`:

```rust
pub struct Body {
    pub basic_blocks: IndexVec<BlockId, BasicBlock>,
    pub local_decls:  IndexVec<LocalId, LocalDecl>,
    pub arg_count:    usize,
    pub span:         Span,
}
```

Cardinality: 4 fields. Layout convention for `local_decls` per
`spec.md:11118-11124`:
```
local_decls[0]                  -> RETURN_PLACE (the return slot)
local_decls[1..=arg_count]      -> argument locals (positional)
local_decls[arg_count+1..]      -> internal temporaries
```

Helper methods (implementation-side, not part of the public
spec interface but expected per `spec.md:11126-11130`):
- `Body::return_local() -> LocalId` returns
  `LocalId::RETURN_PLACE`.
- `Body::arg_locals() -> impl Iterator<Item = LocalId>` yields
  `1..=arg_count`.
- `Body::inner_locals() -> impl Iterator<Item = LocalId>`
  yields `arg_count+1..local_decls.len()`.

**No `Locals` substructure** per `spec.md:11138-11144`. **No
body-level `fn_sig` field** per `spec.md:11146-11149` (lives
Instance-side per Option B). **No body-level `item: ItemId`
back-pointer** per `spec.md:11151-11156`. **No `var_debug_info`
body-level vector** per `spec.md:11157-11161` (consumed at
bridge construction time to populate per-`LocalDecl.debug_name`).

### Public API -- LocalDecl struct

Per `spec.md:11187-11193`:

```rust
pub struct LocalDecl {
    pub ty:         TyId,
    pub mutability: Mutability,
    pub span:       Span,
    pub debug_name: Option<StringId>,
}
```

Cardinality: 4 fields. Per `spec.md:11198-11203`:
- `ty: TyId` -- the local's type (interned via §12).
- `mutability: Mutability` -- `Not` / `Mut` per §10.1.
- `span: Span` -- per-LocalDecl declaration location
  (`Span::dummy()` for synthetic locals introduced by bridge
  collapse / post-construction passes).
- `debug_name: Option<StringId>` -- source-level variable name
  (`Some("temp")`) or `None` for compiler-synthesized rustc
  temps + bridge-introduced synthetic locals.

**Synthetic locals carry `debug_name = None`** per
`spec.md:11219-11224`. Their identity in the IR is purely
positional (LocalId).

### Public API -- LocalDecls allocation API

Per `spec.md:12150-12160`:

```rust
impl Body {
    /// Append `decl` to local_decls; return the freshly-
    /// allocated LocalId. Allocation is positional: LocalId
    /// equals the previous local_decls.len().
    pub fn push_local(&mut self, decl: LocalDecl) -> LocalId;

    /// Convenience helper for synthetic locals (bridge-
    /// introduced + rewrite-pass-introduced). Defaults to
    /// `Mutability::Mut`, `Span::dummy()`,
    /// `debug_name: None`. Returns the freshly-allocated
    /// LocalId.
    pub fn push_synthetic_local(&mut self, ty: TyId) -> LocalId;
}
```

**Single API; no Builder pattern** per `spec.md:12163-12175`.
**No buffering pattern** per `spec.md:12176-12182`. **No
auto-`StorageLive`** per `spec.md:12184-12188` (RegseqModel
dropped `StorageLive` from `StatementKind`).

### Public API -- Visitor trait

Per `spec.md:11442-11472` (located at
`crates/regseq-model/src/visit.rs`):

```rust
pub trait Visitor {
    fn visit_body(&mut self, body: &Body) { self.super_body(body) }
    fn super_body(&mut self, body: &Body) { /* default recursion */ }

    fn visit_basic_block(&mut self, bb: &BasicBlock) { self.super_basic_block(bb) }
    fn super_basic_block(&mut self, bb: &BasicBlock) { /* default recursion */ }

    fn visit_local_decl(&mut self, decl: &LocalDecl) { self.super_local_decl(decl) }
    fn super_local_decl(&mut self, decl: &LocalDecl) { /* default recursion */ }

    fn visit_statement(&mut self, stmt: &Statement) { self.super_statement(stmt) }
    fn super_statement(&mut self, stmt: &Statement) { /* default recursion */ }

    fn visit_terminator(&mut self, term: &Terminator) { self.super_terminator(term) }
    fn super_terminator(&mut self, term: &Terminator) { /* default recursion */ }

    fn visit_place(&mut self, place: &Place) { self.super_place(place) }
    fn super_place(&mut self, place: &Place) { /* default recursion */ }

    fn visit_rvalue(&mut self, rvalue: &Rvalue) { self.super_rvalue(rvalue) }
    fn super_rvalue(&mut self, rvalue: &Rvalue) { /* default recursion */ }

    fn visit_operand(&mut self, op: &Operand) { self.super_operand(op) }
    fn super_operand(&mut self, op: &Operand) { /* default recursion */ }

    fn visit_ty(&mut self, ty: TyId) { self.super_ty(ty) }
    fn super_ty(&mut self, ty: TyId) { /* default recursion */ }

    fn visit_call_callee(&mut self, callee: &CallCallee) { self.super_call_callee(callee) }
    fn super_call_callee(&mut self, callee: &CallCallee) { /* default recursion */ }
}
```

10 `visit_X` / `super_X` hook pairs. Discipline per
`spec.md:11474-11484`:
- **Exhaustive destructuring in `super_X` defaults**: no `..`,
  no catch-all `_` arms. Adding a variant becomes a compile-
  time TODO; new variants are forced into explicit hook
  coverage.
- `super_X` recurses into child constructs, calling `visit_Y`
  for each child.
- Implementors override only the hooks they need; defaults
  handle traversal.

**Mutability discipline** per `spec.md:11486-11493`: read-only
`Visitor` trait at this stage. A `MutVisitor` variant for
transformation passes is a future extension.

### Body-level walk procedure (visit_body default)

Per `spec.md:11500-11524`, the default `visit_body` hook
implements the §24.4.3 6-step DFS-RPO procedure:

```text
visit_body(body):
    // Step 1 (B-NonEmpty pre-check).
    if body.basic_blocks.is_empty():
        emit_error(B_NonEmpty); return

    // Step 2 (B-Successors-Valid pre-check).
    for bb in body.basic_blocks:
        for succ in bb.terminator.successors():
            if succ.as_u32() >= body.basic_blocks.len():
                emit_error(B_Successors_Valid); return

    // Steps 3-5 (DFS-RPO traversal + B-Reachable + B-RPO).
    let rpo = dfs_rpo_from(BlockId::START_BLOCK, body)
    if rpo.len() != body.basic_blocks.len():
        emit_error(B_Reachable)
    for (i, bb_id) in rpo.iter().enumerate():
        if bb_id != BlockId::from_u32(i as u32):
            emit_error(B_RPO); break

    // Step 6 (B-Entry implicit).

    // Per-construct walks.
    super_body(body)
```

The `emit_error` channel is a stub at this stage -- the
visitor trait surface is sited here, but the *concrete*
validator hooks (Place validation per P1a-VAL-01; ID
referential integrity per P1a-VAL-02; layout consistency per
P1a-VAL-03) are implemented by those tasks. A reasonable shape
is to give the trait a default `emit_error(BodyValidationError)`
hook that implementors override; the trait itself does not
need an `emit_error` method if implementors track diagnostics
in their own visitor accumulator.

This task **provides the trait scaffolding and the default
DFS-RPO `visit_body` recursion**; concrete validators in
P1a-VAL-* override `visit_place` / `visit_terminator` etc. to
implement per-construct invariants.

### Conventions

- **Option-B per stage-12 D-12-1.4.** `Body` lives on
  `InstanceKind::Function { body: Option<Body>, ... }`. There
  is no `model.bodies` table, no `BodyId`, no body-level
  `fn_sig`. This task does NOT introduce any of those.
- **`local_decls[0]` is RETURN_PLACE**, `local_decls[1..=arg_count]`
  are argument locals, the rest are temporaries per
  `spec.md:11118-11124`. The helper methods (`return_local`,
  `arg_locals`, `inner_locals`) enforce this convention.
- **`push_synthetic_local` defaults**: `Mutability::Mut`,
  `Span::dummy()`, `debug_name: None`. Document explicitly
  in the function doc comment.
- **`debug_name: None` for synthetic locals** per
  `spec.md:11219-11224`. Their identity is purely positional.
- **No `local_info: ClearCrossCrate<Box<LocalInfo>>` or
  `user_ty: Option<Box<UserTypeProjections>>` carry** per
  `spec.md:11228-11231`. The consumer-relevant
  is-user-variable distinction is discharged via
  `debug_name = Some(_)` vs `None`.
- **No `source_info.scope` carry** per `spec.md:11231-11232`
  (DWARF-emission only).
- **Visitor trait exhaustive destructuring.** All `super_X`
  default impls must use exhaustive destructuring (no `..`,
  no `_`); adding a variant to `Statement`, `Rvalue`, etc.
  forces a compile error in the visitor defaults, surfacing
  the missing hook coverage.
- **B-Body-1 through B-Body-10 invariants** per
  `spec.md:12211-12222` are documented in the visitor trait
  module-level doc comment as forward-pointers to the
  validator tasks (P1a-VAL-01 / P1a-VAL-02 / P1a-VAL-03).
- **No model-level `bodies` table.** The `Body` is constructed
  fresh by the bridge inside `InstanceKind::Function`; the
  validator iterates `model.instances.iter()` and pulls
  `inst.kind.as_function().and_then(|f| f.body.as_ref())`
  per `spec.md:11352-11357`.
- **Serde**: `Body`, `LocalDecl`, `LocalId` are serde-friendly
  (typed IDs + primitives + nested serde-friendly types).
  Use `#[derive(Serialize, Deserialize)]` on `Body` and
  `LocalDecl`. The `Visitor` trait is not serde-bearing.
- **Derive `Clone, Debug, PartialEq, Eq, Hash`** on `Body` and
  `LocalDecl`. (Hash on `Body` may be expensive but is
  required for the project's body-content equality tests; if
  the project chooses to skip Hash on `Body`, document the
  rationale in the type-level doc comment.)
- **Doc comments mandatory** on every public type, field,
  function. Cite §25.x subsections per the spec-traceability
  convention.

## Non-Goals

- **Place validation walk** (`spec.md` §19.7 rules 1-5;
  invariant enforcement in `visit_place`) lives in P1a-VAL-01.
- **ID referential integrity walk** (`spec.md` §3.1 invariants
  1, 2, 6, 9, 11, 18; F4 / F4-2 sort-comparator output check;
  Phase C vtables rebuild + duplicate rejection) lives in
  P1a-VAL-02.
- **Layout consistency + invariants 14, 19, 20 + target/build
  validation** lives in P1a-VAL-03.
- **Per-construct positive invariants** (Operand-site,
  Rvalue-site, Statement-site, Terminator-site,
  BasicBlock-site) live in P1a-VAL-01 / P1a-VAL-02 /
  P1a-VAL-03 walk implementations.
- **Drop-as-Call collapse** (§25.7.1) is bridge-side
  (P1a-BR-06 -- runs at body-construction time per the spec).
- **Vtable-indirect rewrite pass** (§25.7.2) is bridge-side
  (P1a-BR-07 -- runs post-body-construction).
- **DiagLogPeel pass** (§25.7.3) is bridge-side (P1a-BR-08b --
  runs at body-construction time).
- **Bridge per-rustc-local population + named-local lift from
  `var_debug_info`** (`spec.md:11205-11216`) is bridge-side
  (P1a-BR-07).
- **C lowering** for `Body` (§25.10) is phase-1b
  (`generic-c-backend.md` §7).
- **Rendering** (`render_function` body envelope, etc.) lives
  at `pretty-printer.md` §3 / §10.2 and is implemented in
  P1a-PP-05a.
- **`MutVisitor` for transformation passes** -- explicit
  future extension per `spec.md:11486-11493`; not in scope.
- **Cross-Instance B-Body-2 walk** (arg_count matches
  `InstanceKind::Function.fn_sig` input count) per
  `spec.md:12214` is stage-12 / P1a-VAL-02 territory; the
  visitor trait surface here permits the check but does not
  implement it.

## Test Plan

**Test organization**: `unit-in-src` -- tests live in
`crates/regseq-model/src/body.rs` and
`crates/regseq-model/src/visit.rs` `#[cfg(test)] mod tests`.

Coverage requirements (every public type / function / accessor
exercised at least once per `SESSION.md` Testing Requirements):

1. **`LocalId::RETURN_PLACE` const.** Assert
   `LocalId::RETURN_PLACE == LocalId::from_u32(0)`.
2. **`LocalId` size + niche.** Assert
   `size_of::<LocalId>() == 4` and
   `size_of::<Option<LocalId>>() == 4`.
3. **`LocalDecl` construction.** Build with
   `LocalDecl { ty: TyId(0), mutability: Mutability::Mut,
   span: ..., debug_name: Some(StringId(7)) }`; assert
   field-by-field reads.
4. **`LocalDecl` -- synthetic-local default values.** Build
   with `debug_name: None`, `span: Span::dummy()`,
   `mutability: Mutability::Mut`; assert the synthetic-locals
   convention.
5. **`LocalDecl` equality / Hash.** Two structurally identical
   `LocalDecl` are `==` and hash-equal; differ on any field
   -> inequal.
6. **`Body` construction -- empty body.** Build with
   `basic_blocks: IndexVec::new()`,
   `local_decls: IndexVec::new()`, `arg_count: 0`,
   `span: ...`; assert field reads.
7. **`Body` construction -- non-trivial body.** Build with
   one `BasicBlock` (Return terminator, no statements), one
   `LocalDecl` for RETURN_PLACE, `arg_count: 0`; assert.
8. **`Body::push_local` -- positional allocation.** Build an
   empty `Body`; call `push_local(decl_a)` -> assert returned
   `LocalId == LocalId::from_u32(0)`; call `push_local(decl_b)`
   -> assert returned `LocalId == LocalId::from_u32(1)`;
   assert `body.local_decls.len() == 2`.
9. **`Body::push_local` -- entries reachable by index.** After
   the calls in item 8, assert
   `body.local_decls[LocalId::from_u32(0)] == decl_a` and
   `body.local_decls[LocalId::from_u32(1)] == decl_b`.
10. **`Body::push_synthetic_local` -- default values.** Call
    `push_synthetic_local(ty)`; assert the resulting
    `LocalDecl` has `mutability == Mutability::Mut`,
    `span == Span::dummy()`, `debug_name == None`, and
    `ty == <ty>`.
11. **`Body::push_synthetic_local` -- multi-call positional
    allocation.** Two consecutive calls return
    `LocalId::from_u32(0)` and `LocalId::from_u32(1)`.
12. **`Body::return_local`.** For a body with at least one
    LocalDecl, assert `body.return_local() ==
    LocalId::RETURN_PLACE`.
13. **`Body::arg_locals` -- empty arg_count.** With
    `arg_count: 0`, assert
    `body.arg_locals().collect::<Vec<_>>().is_empty()`.
14. **`Body::arg_locals` -- non-zero arg_count.** With
    `arg_count: 3`, assert the iterator yields
    `[LocalId(1), LocalId(2), LocalId(3)]`.
15. **`Body::inner_locals` -- with temps.** With `arg_count: 1`
    and `local_decls.len(): 5`, assert
    `body.inner_locals().collect::<Vec<_>>() ==
    vec![LocalId(2), LocalId(3), LocalId(4)]`.
16. **`Body` equality / Hash.** Two structurally identical
    `Body` values are `==`; differ on any field -> inequal.
17. **`Body` serde round-trip -- empty.**
18. **`Body` serde round-trip -- non-trivial.** Round-trip a
    body with one BasicBlock + one LocalDecl; assert `==`.
19. **`LocalDecl` serde round-trip.**
20. **`Visitor` trait -- default `visit_body` walks.** Build
    a stub `Visitor` impl with a counter that increments in
    `visit_basic_block`. After `visitor.visit_body(&body)`,
    assert the counter equals `body.basic_blocks.len()`.
21. **`Visitor` trait -- `visit_statement` triggers per
    statement.** Build a body with two statements in one
    basic block; counter in `visit_statement` reaches 2.
22. **`Visitor` trait -- `visit_terminator` triggers per
    block.** Counter in `visit_terminator` equals
    `body.basic_blocks.len()`.
23. **`Visitor` trait -- `visit_place` triggers on Assign LHS
    + Rvalue.** Build a body with one Assign statement
    (`Assign(Place::from(LocalId(0)), Rvalue::Use(Operand::Copy(Place::from(LocalId(1)))))`).
    Counter in `visit_place` reaches at least 2 (the LHS Place
    + the Place inside the RHS Operand::Copy).
24. **`Visitor` trait -- `visit_local_decl` triggers per
    local.** Counter in `visit_local_decl` equals
    `body.local_decls.len()`.
25. **`Visitor` trait -- exhaustive destructuring discipline.**
    Compile-time check: a `super_statement` impl with `..` or
    `_` arms would not destructure exhaustively; the project
    convention forbids it. Document via test comment that
    the trait's default impls comply.
26. **`Visitor` trait -- override hook.** Build a custom
    Visitor that overrides `visit_place` to bump a counter;
    confirm that overriding does NOT recurse into Place
    contents (the override replaces the default).
27. **`visit_body` -- `B-NonEmpty` pre-check fires on empty
    body.** A custom Visitor that captures `emit_error`-style
    diagnostics observes a `B_NonEmpty` error when invoked on
    a body with `basic_blocks.is_empty()`. (If the Visitor
    trait doesn't include an `emit_error` channel, this test
    can be a documentation test that the default `visit_body`
    short-circuits cleanly; the concrete validator wiring
    lives in P1a-VAL-01 / P1a-VAL-02.)
28. **No `BodyId` introduced.** Compile-time check: there is
    no `pub struct BodyId` in the project's ID inventory.
    (Searches `crates/regseq-model/src/` for `BodyId` -- the
    only legacy occurrence at `crates/regseq-model/src/instance.rs`
    is replaced by `Option<Body>` per regseq-model-75.)
29. **No `model.bodies` table.** Compile-time check: no
    `bodies: IndexVec<BodyId, Body>` or analogous field on
    `RegseqModel`.
30. **No body-level `fn_sig` field.** Compile-time check via
    struct-init: `Body { basic_blocks, local_decls, arg_count,
    span }` works; adding `fn_sig: ...` yields a compile error.

`cargo test -p regseq-model` passes.

## Review

After tests pass, request a Codex review using the four-block
template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-82 against its task spec at
.harness/tasks/regseq-model/82-body-and-local-decl.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/body.rs
- crates/regseq-model/src/visit.rs
- crates/regseq-model/src/idx.rs (LocalId const-body declaration only)
- crates/regseq-model/src/lib.rs (LocalId size assertions only)

Focus on:
- Spec deviations (every rule in the Implementation Notes section).
- Body has exactly 4 fields (basic_blocks + local_decls + arg_count
  + span); NO body-level fn_sig; NO body-level item back-pointer;
  NO var_debug_info; NO BodyId.
- LocalDecl has exactly 4 fields (ty + mutability + span +
  debug_name).
- push_synthetic_local defaults match spec.md:12155-12159
  (Mutability::Mut, Span::dummy(), debug_name=None).
- Visitor trait has 10 visit_X/super_X hook pairs; super_X
  defaults use exhaustive destructuring (no `..`, no `_`).
- The visit_body default implements the §24.4.3 6-step DFS-RPO
  procedure per spec.md:11500-11524.
- Test coverage vs. the spec's Test Plan, especially the
  layout-convention helpers (return_local, arg_locals,
  inner_locals) and the visitor trait recursion.
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as
hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was
verified and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict,
stop.
</default_follow_through_policy>
EOF
```

Iterate until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

The authoritative gate is `task.py done` per `SESSION.md`.

## Acceptance Criteria

- New module `crates/regseq-model/src/body.rs` created with
  `Body { basic_blocks, local_decls, arg_count, span }` per
  `spec.md:11104-11111`, `LocalDecl { ty, mutability, span,
  debug_name }` per `spec.md:11187-11193`, plus the LocalDecls
  allocation API (`push_local`, `push_synthetic_local`) per
  `spec.md:12150-12160`.
- Helper methods `return_local`, `arg_locals`, `inner_locals`
  on `Body` per `spec.md:11126-11130`.
- `LocalId` typed-index introduced via `newtype_index! { pub
  struct LocalId { const RETURN_PLACE = 0; } }` in
  `crates/regseq-model/src/idx.rs` per `spec.md:11240-11245`.
- `LocalId` size + niche assertions appended to
  `crates/regseq-model/src/lib.rs` per `spec.md:11267-11268`.
- New module `crates/regseq-model/src/visit.rs` created with
  the `Visitor` trait + 10 `visit_X` / `super_X` hook pairs
  per `spec.md:11442-11472`. The default `super_X` impls use
  exhaustive destructuring (no `..`, no `_`).
- The default `visit_body` implements the §24.4.3 6-step
  DFS-RPO procedure per `spec.md:11500-11524`.
- Body / LocalDecl / LocalId / Visitor all re-exported from
  `crates/regseq-model/src/lib.rs`.
- B-Body-1 through B-Body-10 invariants documented in module-
  level doc comments as forward-pointers to P1a-VAL-* tasks.
- All public types carry doc comments citing §25.x
  subsections.
- Test Plan items 1-30 implemented; all pass under
  `cargo test -p regseq-model`.
- `cargo fmt --all -- --check` and
  `cargo clippy -p regseq-model -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
