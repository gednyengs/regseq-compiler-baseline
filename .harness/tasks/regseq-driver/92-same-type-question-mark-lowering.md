# regseq-driver-92: Same-type `Result`-`?` MIR normalizer (Phase-2a)

> Source: the `?`-support design discussion, **doubly reviewed** by Codex
> (independent analysis: `ENDORSE-WITH-CHANGES`). Unblocks the failed
> RegseqError fixture-3 (`?`→`ETIMEDOUT`). Corrects PRD **R02** / GRILL **D07**'s
> wrong assumption that `?` "lowers as ordinary MIR". Memory
> [[question-mark-unbridgeable]].

## Description

The Rust `?` operator is currently **un-bridgeable**. At `mir_opt_level=0` (no
inliner) the `?` desugar is a **fixed shape** with two `core` method calls and an
associated-type-projection-typed cluster:

```
_2 = <Result<(),E> as Try>::branch(_x)              // CORE CALL #1  (#[lang="branch"])
_d = discriminant(_2); switchInt -> [Continue, Break]
  Continue: <bind (_2 as Continue).0>; <original continue successor>
  Break:    _r = (_2 as Break).0;   // Result<Infallible, E>
            _0 = <Result<(),E> as FromResidual<_>>::from_residual(_r)   // CORE CALL #2 (#[lang="from_residual"])
                 -> <original from_residual successor>
```

Two blockers: (1) translating the `branch`/`from_residual` **call signatures**
hits the `Try::Residual`/`Output` associated-type projections, which the bridge
hard-rejects as `NonRigidTy`; (2) the two calls are `core` methods the bridge does
not pull bodies for → `extern` prototypes → **undefined references at link**.

Add a **pre-translation Rust-MIR normalizer** that recognizes this fixed cluster by
**dataflow + lang-item DefIds** and rewrites it to a **direct `Result` discriminant
match** on the `?`-operand — eliminating both core calls, `ControlFlow`,
`Infallible`, and the projection-typed locals. **Scope: same-error-type `?` on
`core::result::Result` only.** This is Codex's recommended Option B (a synthesized
model-body / core-body-pulling approach was rejected — it cannot emit through this
pipeline / drags in `From::from`).

## Input Context

### Code anchors (confirm at HEAD)

- `crates/regseq-driver/src/bridge/body/materialise.rs:289-295` — the owned,
  substituted MIR body (the `instance_mir` fetch is `:288`). The normalizer MUST run
  here, **before** local-decl translation (`:315`) and basic-block translation
  (`:331`). The existing peel pass (`diag_log_peel_pass`, `materialise.rs:669`) runs
  **after** translation — **too late**, because by then the core callees have already
  been translated.
- `crates/regseq-driver/src/bridge/body/terminator.rs:793`/`:937` —
  `translate_call_callee` / `callsite_fn_sig`: the path the `branch`/`from_residual`
  callees would hit (and trigger blocker 1/2) **if not consumed first**.
- `crates/regseq-driver/src/bridge/ty.rs:169`/`:207`/`:484` — the projection folder /
  allowlist / `NonRigidTy` `Alias` reject that Option B **sidesteps** (no new
  projection authorization needed).
- `crates/regseq-driver/src/callbacks.rs:25` — the cleanup passes
  (`SimplifyLocals-final`, `ReorderBasicBlocks`, `ReorderLocals`) **renumber locals
  and blocks**, so the normalizer must match by **structure/DefIds, not numeric IDs**.
- Lang items (pinned nightly, `library/core/src/ops/try_trait.rs`): `#[lang =
  "branch"]` (`:217`) and `#[lang = "from_residual"]` (`:331`) → identify the callees
  via `tcx.is_lang_item(callee_def_id, …)` (same mechanism as the `CVoid` arm in
  `ty.rs`). Semantics: `branch` = `match self { Ok(v)=>Continue(v), Err(e)=>Break(Err(e)) }`
  (`result.rs:2172`); `from_residual` = `match r { Err(e)=>Err(From::from(e)) }`
  (`result.rs:2187`); reflexive `From<T> for T = { t }` (`convert/mod.rs:788`) — so
  for **same-type** `?`, `from_residual` is the identity `Err(e)→Err(e)`.

## Implementation Notes

- **Recognize by dataflow:** a `branch` call (lang item) writing `dst`; a
  `discriminant(dst)` + `switchInt` selecting `Continue`/`Break`; the `Break` arm
  extracting `(dst as Break).0` then calling `from_residual` (lang item) into the
  return place. Accept arbitrary local/block IDs.
- **Continue payload bind is shape-variable:** for a non-unit `Ok` the Continue arm
  binds `(dst as Continue).0`, but for a **unit/ZST `Ok`** the `RemoveZsts` cleanup
  pass (`callbacks.rs:25`) elides the payload write, so there is **no bind**. The
  recognizer must accept **both** the unit no-bind shape and the non-unit bind shape.
- **Rewrite preserving successors + drops (Codex blocking #2):** replace the cluster
  with a discriminant match on the **`?`-operand** (`branch`'s argument): on `Ok`,
  bind the payload and `goto` the **original continue successor**; on `Err`, set
  `_0 = Err(<err payload>)` and **`goto` the original `from_residual` successor**
  (NOT a synthesized bare `return` — an early-return path may drop a live local; the
  bridge supports Drop-as-Call).
- **Same-type gate (type level):** rewrite only when the `?`-operand is
  `Result<_, E_src>` and the function returns `Result<_, E_dst>` with `E_src ==
  E_dst` (after region erasure/normalization). 
- **Fail loud (explicit diagnostic, not a vague `NonRigidTy`)** for: cross-type `?`
  (`E_src != E_dst`, i.e. a real `From` conversion), `Option`-`?` (different carrier),
  any shape mismatch, and a `needs_drop` `Ok` payload the rewrite cannot
  faithfully preserve.

## Non-Goals

- Cross-type `?` (`From` conversion) and `Option`/other-carrier `?` — **deferred**
  (fail-loud).
- No projection-allowlist change; no pulling of `core` bodies; no global inliner
  (would perturb every committed golden — rejected for blast radius).
- No RegseqError fixtures here (those are `regseq-backends-35`/`-36`).

## Test Plan

1. Bridge tests over fixtures exercising same-type `?`:
   - `?` with a **discarded** unit `Ok` (`helper()?;`);
   - `?` with a **used non-unit** `Ok` (`let v = helper_val()?;`);
   - **multiple** `?` in one body;
   - an **early-return path with a live local** (drop preservation).
   Each bridges (no `NonRigidTy`, no `body:None` core call) and the resulting model
   is the direct discriminant-match shape (no `branch`/`from_residual`/`ControlFlow`).
2. Fail-loud: a **cross-type** `?`, an `Option`-`?`, **and a same-type `?` with a
   `needs_drop` `Ok` payload the rewrite cannot preserve** (e.g. `Result<String, E>`)
   each produce the explicit unsupported-`?` diagnostic (not `NonRigidTy`).
3. End-to-end: a same-type `?` fixture emits + `cc` + runs with the expected exit.
4. Existing bridge/backend tests + goldens byte-identical (**I02**); gate `pass`.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-driver`, `regseq-backends`,
`codegen-coverage-runner`); `RUST_TEST_THREADS=1 cargo test -p regseq-driver
-p regseq-backends -p codegen-coverage-runner -- --test-threads=1`; gate `pass`;
ledger byte-stable.

## Acceptance Criteria

1. Same-type `Result`-`?` bridges + lowers to a **direct discriminant match** with
   **no** `branch`/`from_residual`/`ControlFlow`/`Infallible`/projection in the model;
   no `body:None` core call (no link failure).
2. The rewrite **preserves** the original continue/break successors and any drops
   (no synthesized bare return).
3. Cross-type `?`, `Option`-`?`, shape mismatch, and unpreservable `needs_drop`-`Ok`
   **fail loud** with an explicit unsupported-`?` diagnostic.
4. Matching is by structure/lang-item DefIds (robust to the cleanup passes'
   renumbering); the edge-case tests (unit/non-unit/multiple/drop) pass.
5. Existing goldens byte-identical (**I02**); gate `pass`; fmt + scoped clippy clean.
