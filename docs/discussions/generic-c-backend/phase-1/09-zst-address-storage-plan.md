# Plan 09 — storage for address-taken ZST locals

> **Status:** plan-first (Codex-reviewed before implementation).
> **Scope:** one generic-C backend codegen bug. No surface expansion.

## Problem

A non-capturing closure passed **by value** to a generic function fails to
compile after emit. Minimal reproduction (within the supported surface):

```rust
impl Point {
    fn modify_x<F: Fn(i32) -> i32>(&mut self, f: F) { self.x = f(self.x) }
}
// ...
p.modify_x(|x| x + 2);
```

`cc` rejects the emitted TU:

```
generic.c: In function '...modify_x...':
error: '_2' undeclared (first use in this function)
    _3 = &(_2);
```

(This surfaces only after Plan-09's sibling fix — forcing `panic=abort` in the
production driver callbacks — let the program bridge at all. The two are
independent: the bridge fix is committed separately.)

## Root cause

The closure `|x| x + 2` captures nothing, so its type `F` is a **zero-sized
type** (the emitted struct is `_Static_assert(sizeof(...) == 0)`). `modify_x`'s
MIR takes the closure's address to pass as `Fn::call`'s receiver:

```
_3 = &_2;                                  // _2: F  (the ZST closure arg)
_6 = <F as Fn<(i32,)>>::call(move _3, ...) // receiver is &_2
```

Two backend rules collide:

1. **`fn_signature` elides ZST parameters** from the C signature
   (`generic_c/mod.rs:939` — `if is_zst(eff) { continue }`). Correct: a ZST
   occupies no bytes, so it carries no ABI slot.
2. **`emit_body` declares no storage for a ZST local** — all three declaration
   phases skip ZSTs (`generic_c/body_emit.rs:103` for `_0`, `:122` for
   substituted args, `:135` for inner locals). Also correct in general: a ZST
   value read/write is a no-op, so the local needs no C object.

But a **Direct ZST argument** is therefore declared *nowhere* — neither a C
formal (rule 1 elided it) nor a body local (rule 2 skipped it). When the body
takes its address (`&_2`), `ref_expr` emits `&(_2)` (`body_emit.rs:1344`)
against a name that does not exist → malformed C.

The general invariant the backend currently violates: **a ZST local whose
address is taken needs real (zero-sized) storage to point at.** Rust's `&zst`
is a valid aligned, never-dereferenced pointer; the C analogue is the address
of a declared empty struct (zero-sized under `-std=gnu11`).

## Fix

Declare a ZST local **iff its own storage is borrowed by a bare-local borrow**
— the local appears as the `place` of a `Rvalue::Ref` / `Rvalue::RawPtr` with an
**empty projection** (`&local`, exactly the failing `&_2`). This is the minimal
scope that fixes the reported bug.

**Out of scope (deliberately):** a borrow *through* a projection — `&(*ptr)...`
(re-rooted at the pointee, already a declared pointer) and `&local.zst_field` /
`&local[i]` (a borrow of a *projected* ZST place). The latter is a **separate,
pre-existing** concern: a ZST field is omitted from the emitted aggregate
(`layout_emit.rs:311`) while `place_walk` still spells `.field`
(`body_emit.rs:1159`), so declaring the base local would not make
`&(local.zst_field)` valid C. No coverage fixture exercises it, it is not the
reported bug, and this fix leaves that path exactly as-is (the empty-projection
scope never marks a projected place). [Codex plan-review must-fix.]

### Mechanism

1. New read-only helper on `BodyEmit` — collects only **bare-local** ZST borrows:

   ```rust
   /// ZST locals whose own storage is borrowed by a bare-local `&local`
   /// (empty projection). Such a local must be given real zero-sized
   /// storage so `&` has a valid, never-dereferenced target. A borrow
   /// through any projection (`&(*p)`, `&l.f`, `&l[i]`) is excluded: it is
   /// either re-rooted at a pointee or a separate projected-ZST concern.
   fn bare_borrowed_zst_locals(&self) -> HashSet<LocalId> {
       let mut set = HashSet::new();
       for bb in self.body.basic_blocks.iter() {
           for stmt in bb.statements.iter() {
               if let StatementKind::Assign(_, rv) = &stmt.kind {
                   if let Rvalue::Ref(_, p) | Rvalue::RawPtr(_, p) = rv {
                       if p.projection.is_empty() && self.is_zst(self.body.local_decls[p.local].ty) {
                           set.insert(p.local);
                       }
                   }
               }
           }
       }
       set
   }
   ```

   (`Ref`/`RawPtr` only appear in `Assign` rvalues — `Operand`s in terminators
   are `Copy`/`Move`/`Const`, never a borrow [Codex confirmed: `terminator.rs:84`,
   `operand.rs:30`] — so statements are the complete scan surface.)

2. In `emit_body`, compute the set once and, in each declaration phase, declare
   a ZST local that is in the set (instead of skipping it). The declared type is
   the local's own type via `types::declare`; a gnu11 empty struct is
   zero-sized, so `&` yields a valid, never-dereferenced pointer — matching the
   Rust semantics. Covers Direct ZST args (the failing case), substituted ZST
   args, inner ZST locals, and (for completeness) a ZST `_0`.

No change to `fn_signature`: ZST args stay elided from the C ABI. The address-
taken ZST arg becomes a plain uninitialized body local; this adds **only a
declaration, no prologue marshalling** — ZST args carry no bytes and are already
skipped by the `Substituted`-input prologue (`body_emit.rs:515`), so no copy is
synthesized. The callee never reads the local's bytes.

## Why this is determinism-safe for the existing corpus

The 35 coverage fixtures each pass the **host-`cc` compile + run** leg. Any
fixture that bare-borrowed a ZST local (`&local`) would today emit
`&(local)` against a name declared nowhere (signature-elided at `mod.rs:939`,
declaration-elided at `body_emit.rs:122`/`:135`) and fail that leg with the
undeclared-identifier error above. Since all 35 are green, **none bare-borrow a
ZST local**, so `bare_borrowed_zst_locals()` is empty for every existing fixture
→ zero new declarations → every `golden.c` is byte-identical. (The helper is
deliberately ZST- and bare-scoped, so the abundant non-ZST and projected borrows
in the corpus never enter the set.) The gate stays `pass` with no `--bless`; new
behavior manifests only for the new closure-by-value fixture.

## Alternatives considered

- **Emit `&(T){}` (compound-literal address) for a ZST borrow** instead of
  declaring the local. Rejected: the codebase's borrow path is uniformly
  `&(lvalue)`; a value-vs-storage split at the borrow site is more invasive
  than one declaration, and a compound literal's lifetime is its enclosing
  block — correct here but a subtler contract than a block-scoped local.
- **Always declare every ZST local.** Rejected: churns goldens (every unit `()`
  / ZST temp gains a declaration) and adds unused-variable noise for the
  common no-address case.

## Verification

1. The Plan-09 fixture — a `kind=rust` coverage fixture exercising a
   non-capturing closure passed by value (`Fn(i32) -> i32`), asserting stdout —
   added to `crates/codegen-coverage-runner/fixtures/`. New row: emit → `cc` →
   run → stdout oracle, plus a blessed `golden.c`.
2. `cargo run -p codegen-coverage-runner` → verdict `pass`, 35→36 green, run
   twice + `diff` the ledger json (byte-stable).
3. The real-CLI repro (`modify_x` / `Point`) compiles + links + runs via
   `cargo regseq --emit bin`.
4. `cargo test -p regseq-backends` + the workspace suite green; fmt + clippy
   (`--workspace -D warnings`) clean.
5. Codex diff review.
