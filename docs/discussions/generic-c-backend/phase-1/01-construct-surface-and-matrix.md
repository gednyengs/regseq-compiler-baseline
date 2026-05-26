# Phase 1 · Doc 1 — Construct surface + codegen-coverage matrix

**Status: SKELETON — the re-derivation below is the next action (fresh
session, full budget).** This document is the authoritative enumeration
of every `RegseqModel` construct the generic-C backend must consume, each
with its pinned C lowering and a coverage status against the canonical
spec. It is the **backbone** of Phase 1 — Docs 2 and 3 reference it.

## Method (how to fill this — do NOT shortcut)

Re-derive the surface **exactly**, from two sources, and reconcile:

1. **The owned IR** — `crates/regseq-model/src/*` enum/struct definitions
   (the source of truth for what variants exist). Walk: `item.rs`,
   `instance.rs`, `ty.rs`, `layout.rs`, `const_value.rs`, `place.rs`,
   `operand.rs`, `rvalue.rs`, `statement.rs`, `terminator.rs`,
   `basic_block.rs`, `body.rs`, `fn_sig.rs`, `cabi.rs`, `domain_attr.rs`.
2. **The canonical C-emission spec** —
   `../../regseq-model/generic-c-backend.md` (what C lowering is already
   pinned) and `../../regseq-model/spec.md` (the producer-side rules +
   §14.5 C-ABI admissibility).

For **every** variant/field, record one matrix row. Classify its spec
coverage; the `unspecced`/`deferred` rows are the gap list that drives
Doc 2 + the spec delta. This supersedes the earlier exploratory agent
maps — re-derive against source, do not copy them.

## Coverage legend (the `status` column)

- **covered** — `generic-c-backend.md` pins this construct's C lowering.
- **deferred** — the spec explicitly defers it (e.g. §7 terminators).
- **unspecced** — no C lowering pinned anywhere; a genuine gap to resolve
  in Doc 2 and write into the spec delta.
- **out-of-scope** — not lowerable in Phase 1 by design (e.g. unbridged
  domain constructs; rustc-rejected shapes) — record with the reason.

## Matrix schema

One row per construct:

| id | construct (variant/field) | C lowering | status | spec ref | fixture | notes |
| -- | ------------------------- | ---------- | ------ | -------- | ------- | ----- |

(`id` = a stable per-row tag, family-prefixed, à la the bridge matrix.
`fixture` = the codegen-coverage fixture that exercises it — see Doc 3.)

## Construct families to enumerate (fill each subsection)

> Each heading below becomes a matrix block. The parenthetical is the
> source file + the spec section that should cover it; "GAP?" flags where
> a hole is anticipated.

### 1. Items — `ItemKind` + emission-relevant fields
`item.rs`, spec §8. Variants (Function/Static/Const/Struct/Enum/Union) +
fields the backend needs: `linkage`, `link_section`, `inline_hint`,
`opaque_handle_eligible`. *(TO FILL.)*

### 2. Instances + `InstanceKind`
`instance.rs`, spec §9. What the backend emits per instance (the backend
walks **instances**, post-mono, not items). Foreign (`body: None`) →
prototype/extern; defined/shim → definition. *(TO FILL.)*

### 3. `Ty` (19 variants) → C type
`ty.rs`, spec §10; lowering pinned at `generic-c-backend.md` §2 (+ §3 for
ref/ptr/fat-pointer). Expect mostly **covered**. *(TO FILL — confirm each.)*

### 4. `Layout` → struct/enum/union/tuple emission
`layout.rs`, spec §11. Field offsets, enum `EnumShape`/`TagEncoding`
(Direct vs Niche), union, packed. *(TO FILL — note where §2's enum
emission is example-only vs pinned.)*

### 5. `Const` / `Byte` vocabulary → C initializer
`const_value.rs`, spec §9.7. Bool/Char/Int/Uint/Float/ZST/Bytes/Aggregate/
EnumValue/NullPtr/Reloc + Byte::{Uninit,Value,Reloc}. Partly via §5
(`Operand::Constant`). **GAP?** per-variant C initializer forms. *(TO FILL.)*

### 6. `Place` / `PlaceElem` → C lvalue
`place.rs`, spec §19; pinned at `generic-c-backend.md` §4. **covered**
expected. *(TO FILL — confirm.)*

### 7. `Operand` → C
`operand.rs`, spec §20; pinned at §5. **covered** expected. *(TO FILL.)*

### 8. `Rvalue` (+ `BinOp`/`UnOp`/`CastKind`/`AggregateKind`) → C expr
`rvalue.rs`, spec §21. **GAP?** `generic-c-backend.md` does not appear to
fully pin per-Rvalue / per-BinOp / per-CastKind C emission — likely a
large `unspecced` block to resolve. *(TO FILL — this is a priority gap.)*

### 9. `Statement` → C
`statement.rs`, spec §22. `Assign` / `SetDiscriminant` / `Intrinsic`
(`Assume`, `CopyNonOverlapping`). Intrinsics pinned at §6;
`SetDiscriminant` (tag write) **GAP?**. *(TO FILL.)*

### 10. `Terminator` (6 variants + `AssertKind`) → C  ⚠ KNOWN DEFERRED
`terminator.rs`, spec §23; `generic-c-backend.md` §7 **explicitly defers
all six** (`Return`/`Goto`/`SwitchInt`/`Call`/`Assert`/`Unreachable`, +
MIR `Drop` as a synthetic `Call`). Must be **fully pinned** here +
resolved in Doc 2 + written into the spec delta. *(TO FILL — core P1 work.)*

### 11. `FnSig` / `FnAbi` + C-ABI admissibility → C signatures
`fn_sig.rs`, `cabi.rs`, spec §14 (esp. §14.5 + `CAbiLowering`/`CAbiShape`).
How signatures + `extern "C"` boundary lowering are emitted. *(TO FILL.)*

### 12. Diagnostic-logging domain construct → `printf`
`domain_attr.rs` (`AttrData::Intrinsic(IntrinsicKind::Log)`); pinned at
`generic-c-backend.md` §8 (per-level call, per-arg specifier, `&str`
NUL/`%.*s`, `max_str_args_per_call`). Backend rule (locked): the
log-marked item is a **recognition tag — intercept at call sites; emit no
C definition for it**; args arrive already peeled flat. *(TO FILL —
confirm §8 completeness + the interception mechanics, which cross-ref Doc 2.)*

## Output of this doc

1. The filled matrix (every construct, classified).
2. **The gap list** — all `unspecced` + `deferred` rows — which is the
   agenda for Doc 2 (architecture decisions) and the `generic-c-backend.md`
   spec delta.
