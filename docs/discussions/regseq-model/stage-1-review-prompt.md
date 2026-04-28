# Stage 1 Review Prompt (Round 5)

A self-contained prompt for an external reviewer (human or LLM) to
adversarially assess the **round-5 revisions** to the Regseq stage-1
vocabulary specification. The reviewer has no access to the project
repository; everything they need is in the attached files referenced
under "Attachments."

This supersedes the round-3 prompt (the prior contents of this file
introduced at commit `d6bfcc4`; recoverable via git if needed). Round
5 introduced a meaningful pivot in the monomorphization story and the
C-semantics framing of the IR; this review focuses on those specific
revisions, not on the spec as a whole.

To use this: attach the files listed under "Attachments" alongside
this prompt and send to the reviewer.

---

## Role

You are conducting an adversarial review of the round-5 revisions to
a specification document for an internal compiler IR. Round 5
restructured the monomorphized-item story (Instance/InstanceKind/
origin/interning), removed the prior `entry_points` table, pinned a
"C-semantics with information preservation" framing, and pinned the
lifting of `impl` blocks. Your job is to find issues in **these
specific revisions** that would cause an implementer to either build
the wrong thing or stall on ambiguity, or that introduce
inconsistencies with prior review rounds (R1-R4) that have already
shipped.

Be skeptical and specific. Vague concerns are not useful;
cite-and-fix concerns are. You have no access to the project
repository; if the spec cites a file not in the attachment set,
treat it as out of scope.

## Project framing

Regseq is a Rust-to-C compiler for OS-agnostic device drivers.
Driver authors write `#![no_std]` Rust specs (with a `#[rseq::]`
attribute namespace and an embedded DSL); the compiler consumes
rustc's `optimized_mir` (with `mir_opt_level=0` and a curated
cleanup-pass whitelist) and produces C for target operating systems
(Linux, Zephyr).

The bridge between MIR and the C backend is an internal IR called
`RegseqModel`. The IR is specified bottom-up in 13 stages. The
stage-1 (vocabulary) spec has gone through four prior review rounds
documented in `stage-1-review-log.md`; this round-5 review focuses
on the specific revisions introduced after round 4 closed and before
step-5 implementation begins.

`RegseqModel` is fully owned (no rustc lifetimes survive past the
bridge call), single-level (no IR ladder; stages refine the same
IR), and serializable (custom textual MIR-inspired pretty-printer
plus serde JSON). Its two downstream stakeholders are domain
analyses and the C codegen backend.

## Round-5 scope

Round 5 revised the spec on six points, documented as R5.1 through
R5.6 in `stage-1-review-log.md`. In summary:

- **R5.1 (Instance shape).** The thin `Instance { item, args }`
  was replaced with a richer `Instance { item, generic_args,
  symbol_name, span, kind }` plus a closed six-variant
  `InstanceKind` enum (Function, Static, Const, Struct, Enum,
  Union). The interning rule was tightened to key on full rustc
  instance identity (`InstanceKind` + args), not just
  `(item, generic_args)` -- preventing rustc shims that share a
  `DefId` (DropGlue, ReifyShim, FnPtrShim, VTableShim, etc.) from
  collapsing into a single RegseqModel `Instance`. The shim
  discriminator is **not** exposed on the public type; it survives
  in `symbol_name`, `body`, and the F4-2 sort key.

- **R5.2 (Roots).** `RegseqModel.entry_points: Vec<ItemId>` was
  removed. Construction invariants were renumbered. Roots are
  consumer-derived predicates over `instance.item`'s attributes /
  ABI / visibility (which live on `Item`, stage 12).

- **R5.3 (Rust → C semantics framing).** A new methodology rule
  was added to `memory.md` ("C semantics with information
  preservation") and a new "Rust → C lowering reference" section
  was added to `stage-1-vocabulary.md`. Together, they pin that
  RegseqModel is a C-shaped IR, the bridge does the desugaring
  (drop glue → regular Calls; trait objects → vtables; closures
  → structs + methods; etc.), and source-side annotations are
  translated per-construct (not passed through opaquely).

- **R5.4 (`impl` lifting).** `impl` blocks are source-level
  scopes only; no `ItemKind::Impl`. Methods lift to top-level
  Functions with explicit receivers. Trait declarations and
  associated types are erased; static dispatch resolves directly,
  dynamic dispatch via synthesized vtables (Static Instances).
  `ItemKind` (stage 12) is pinned as a six-variant enum mirroring
  `InstanceKind`. `DefPathData::Impl` survives only as path scope
  for diagnostics.

- **R5.5 (Synthesized source-file sentinel).** `SourceFileId::ZERO`
  reserved for the synthesized sentinel (`path = "<synthesized>"`,
  `content: None`, `line_starts: vec![0]`). Bridge-synthesized
  Instances (drop glue, vtable shims, closure-capture structs)
  use `Span { file: SourceFileId::ZERO, lo: 0, hi: 0 }`.
  Construction invariant added.

- **R5.6 (InstanceKind explicit).** Six explicit variants over a
  collapsed form. Field rename `args` → `generic_args`.
  `Static.ty` and `Const.ty` distinct (different C lowerings:
  static gets a global, const can be `#define`-style or
  constexpr-baked).

These six points are the round-5 scope. R5.1 is HIGH-impact; R5.2
through R5.6 are MEDIUM-LOW.

## Attachments

| File | Role for this review |
|---|---|
| **PRIMARY (must read)** | |
| `stage-1-vocabulary.md` | The revised specification. The new "Rust → C lowering reference," "Lifting `impl` items," "Instances: monomorphized items" (incl. `Instance`, `InstanceKind`, interning rule, F4-2 ordering, sentinel rule), the updated `RegseqModel` struct, the renumbered construction invariants, and the updated conventions table are the round-5 substance. |
| `stage-1-review-log.md` | Rounds 1-4 already addressed; do not re-flag those issues. Round-5 entries (R5.1-R5.6) document the new revisions and their rationale. The "Notes for the next reviewer" section flags areas not yet adversarially probed. |
| `memory.md` | Project briefing and methodology rules. The new "C semantics with information preservation" rule is the foundational policy that R5.3 codifies. The pre-existing rules (`core` is opaque, no defensive fallbacks, compiler-theory-first, etc.) still apply. |
| `architecture.md` | Q1-Q7 architectural decisions the spec sits inside. SETTLED. R5 must not contradict any of them. Q2 ("eager translation; no `'tcx` past the bridge") and Q3 ("single-level IR") are particularly load-bearing for round 5. |
| **STRONGLY RELEVANT** | |
| `stage-1-research.md` | F1-F4 sub-decisions. F2 (spans inline, no SpanId) and F4 (item determinism) interact with R5.5 (sentinel) and R5.1 (F4-2 ordering). |
| `regseq-background.md` | DSL background; useful for understanding what driver-spec source actually looks like and what consumers (analyses + codegen) need from RegseqModel. |
| `rustc-excerpts.md` | Curated quotations from the pinned rustc-src checkout. Treat as authoritative for rustc-internal claims (especially `InstanceKind` taxonomy, drop glue, vtables, `SymbolName`). R5.1's interning correctness depends on these. |
| `status.md` | Current state of stage-1 implementation. Steps 1-4 have shipped (commits `b8f1e67`, `fd0d3c8`, `82aaaed`, `f34df65`); step 5 is queued behind this review. |
| **OPTIONAL (use for cross-checks)** | |
| `prior-art/README.md` | Synthesis of prior-art research that informed the architectural decisions. |
| `prior-art/rustc-public.md` | The "BodyBuilder + monomorphization" pattern R5 leans on. |
| `prior-art/charon.md`, `prior-art/llvm-ir.md`, `prior-art/rustc-codegen-c.md`, `prior-art/kani.md` | Per-project prior art. Use to verify any "X does Y" claim before challenging. |

Read order if approaching cold: `memory.md` first (briefing +
methodology), then `architecture.md` (Q1-Q7), then
`stage-1-review-log.md` Round 5 (the *what changed* and *why*),
then `stage-1-vocabulary.md` (the spec, with R5 sections in mind).
The other files are reference material; consult on demand.

## Out of scope

Do not flag any of the following:

- **Issues already addressed in R1-R5 of `stage-1-review-log.md`.**
  Re-flagging these wastes effort.
- **Q1-Q7 architectural decisions** in `architecture.md`. Settled.
  R5 is reviewed *against* them; they are not subject to review.
- **F1-F4 sub-decisions** in `stage-1-research.md`. Settled.
- **Stage 3-13 IR shapes.** R5 forward-references types, bodies,
  consts, items -- those are out of stage-1 scope. Only flag
  inconsistencies between R5 and what stages 3-13 will need from
  stage 1.
- **Bridge translation logic.** Specified separately; not in the
  attachment set.
- **Stylistic preferences** (naming variations, doc-comment
  phrasing). Focus on substance.
- **Implementation work already shipped** (steps 1-4 in
  `crates/regseq-model/`). The crate source is not in the
  attachment set; treat it as out of review scope.
- **The `entry_points` removal itself.** R5.2 already pinned this;
  re-litigating the removal is out of scope. Flag only if R5.2
  introduces a new gap *because* of the removal (e.g., a dangling
  reference, a missing rule a consumer needs).

## Probe categories

Review across these axes. For each, the question to ask of the
round-5 revisions:

### 1. Instance shape coherence (R5.1, R5.6)

Is the new `Instance` shape internally consistent? Does the
interning rule (key on full rustc instance identity, surface
`(item, generic_args)` to consumers) preserve correctness without
leaking rustc taxonomy into the public type? Are the six
`InstanceKind` variants closed (no missing rustc cases that
matter for Regseq drivers), and is each variant's forward-reference
contract (BodyId / TyId / ConstId) sound? Does `symbol_name`
uniquely identify a rustc instance, given the discriminator is
not exposed?

Specific probes:
- Two call sites resolving to `Item(foo)` and `ReifyShim(foo)` --
  do they correctly become distinct `Instance`s under the spec?
- A driver that uses `static FOO: u32 = 7;` and `static mut BAR:
  u32 = 0;` -- does the spec disambiguate them clearly? Where
  does mutability live?
- Drop glue for a generic struct -- is the `Instance` shape
  workable? Does `Origin = (item, generic_args)` correctly point
  at `core::ptr::drop_in_place` plus `[Type(T_id)]`?

### 2. C-semantics framing (R5.3)

Is the methodology rule in `memory.md` internally consistent and
free of contradictions with the existing rules? Does the
"Rust → C lowering reference" table in `stage-1-vocabulary.md`
match the methodology principle, or do any rows pass through
source-side annotations as opaque metadata?

Specific probes:
- `&mut T` → `T* restrict`. Is the `restrict` claim sound (rustc's
  borrow uniqueness genuinely matches C's `restrict` contract)?
  Cross-check the rustc reference rules.
- `repr(C)` → "resolved Layout, not the annotation." Does this
  actually buy us anything over passing the annotation? What
  about `repr(transparent)`, `repr(packed)`, custom alignments?
- Is the `impl`-lifting principle compatible with the
  "C semantics" rule, or does lifting throw away information that
  C codegen / analyses would benefit from?

### 3. `impl` lifting (R5.4)

Is the lifting principle complete? Does it cover every kind of
content an `impl` block can hold? Does it produce a
self-consistent flat IR for the cases that matter for `no_std`
driver code?

Specific probes:
- Default-method bodies on traits (`trait Foo { fn bar() { ...
  } }` with no overriding `impl Foo for X { ... }`). Does the
  default body get monomorphized into `Instance`s correctly?
- Trait objects + vtable construction. The spec says vtables are
  Static Instances of fn-ptr-struct type; `tcx.vtable_entries`
  gives the entries. Is anything missing for a complete vtable
  story?
- Inherent impls on generic types (`impl<T> Vec<T> { fn push }`).
  Does the lifted `Vec_push::<T>` Instance work cleanly?
- `Self` reference in method signatures. Substitution erases
  `Self` post-monomorphization; is this faithfully captured?

### 4. Synthesized source-file sentinel (R5.5)

Is the sentinel rule consistent with the existing `SourceFile`
shape and `Span::slice` behavior? Does it conflict with any prior
spec text or implementation contract?

Specific probes:
- `SourceFile.line_starts: vec![0]` for the sentinel — is this
  the right shape (vs. `vec![]`)? The current `SourceFile::position`
  implementation behavior on different `line_starts` shapes is
  documented; verify the sentinel works under it.
- Construction invariant 8 says real source files start at
  `SourceFileId(1)`. Is this enforceable by the bridge?
- Does the methodology rule about `core is opaque` create any
  tension with synthesized spans pointing at "core" origins?

### 5. F4-2 Instance ordering (R5.1)

Is the F4-2 sort key strong enough to produce stable
`InstanceId`s across builds? Does it correctly tiebreak the
shim-discriminator case?

Specific probes:
- Two builds where `tcx.instance_mir(Item(foo, [u32]))` and
  `tcx.instance_mir(VTableShim(foo, [u32]))` both appear -- do
  they get deterministic, distinct InstanceIds?
- The `instance_kind_disambig: u8` is described as "fixed by the
  bridge with the same append-only rule as F4." Is the spec
  precise enough about who maintains this rank table?

### 6. Forward-fit with stages 3-13

R5 forward-references `BodyId`, `TyId`, `ConstId`, the
`ItemKind` six-variant enum, etc. Are these forward-references
consistent with what the corresponding stages will need?

Specific probes:
- Stage 3 (Ty) needs to define `Ty::Struct`, `Ty::Enum`,
  `Ty::Union`. Does R5's `InstanceKind::Struct { layout: TyId }`
  pattern fit cleanly?
- Stage 11 (Body) needs to define `Body` and possibly `FnSig`.
  Does R5's `InstanceKind::Function { body: Option<BodyId> }`
  leave room for stage 11 to add what it needs (e.g., a sig
  field on Instance separate from Body)?
- Stage 12 (Item) needs `ItemKind` with attributes/linkage/etc.
  R5 implies a six-variant `ItemKind` mirroring `InstanceKind`.
  Is anything missing that stage 12 will be forced to add as a
  seventh variant?
- Stage 13 (domain extensions) is allowed to extend
  `InstanceKind`. Is the extension surface clean enough?

### 7. Cascade and consistency

Did R5 introduce any inconsistency with rounds 1-4? Did the
removal of `entry_points` create dangling references? Is the
new construction-invariant set complete (cover what was lost)?

Specific probes:
- Search the spec for any reference to `entry_points` or "entry
  points" that survived the removal.
- Does the spec still say "items are sorted at construction" (F4)
  in a way that's compatible with F4-2?
- Are there any lingering references to the old `Instance { item,
  args }` shape (e.g., in pretty-printer conventions, conventions
  table, etc.)?

### 8. Implementation feasibility (round-5 specific)

Could a competent Rust engineer implement step 5 from this spec?
Where does R5 under-specify something an implementer would need
to invent?

Specific probes:
- The interning HashMap's key shape is described abstractly
  ("full rustc instance identity"). Is this implementable in
  `regseq-driver`? rustc exposes this through its own `Instance`
  type.
- `tcx.symbol_name(instance)` is named as the source of
  `symbol_name`. Is this rustc API stable enough for the pinned
  toolchain?
- `instance_kind_disambig: u8` rank table: where does the
  authoritative list live? Stage 1 defers to stage 11; is that
  satisfactory?

## Grounding rules

- **Cite file and section for every claim.** Format:
  `stage-1-vocabulary.md "Lifting impl items"` is acceptable.
  Line numbers are not required.
- **Treat `rustc-excerpts.md` as authoritative** for any
  rustc-internal claim. Do not invent rustc behavior.
- **Mark hypotheses as hypotheses.** Prefix any claim you cannot
  verify against an attached file with `HYPOTHESIS:`.
- **References outside the attachment set are out of scope.** If
  the spec cites `crates/regseq-model/` or `scripts/...`, do not
  chase those.
- **No issues found in a category is a valid finding.** Do not
  invent issues to fill probe categories.

## Output contract

Emit exactly this shape (no preamble, no postscript, no
restating-the-prompt):

```
VERDICT: <APPROVE_AS_IS | APPROVE_WITH_MINOR_REVISIONS | REVISE_BEFORE_IMPL | REJECT_AND_RETHINK>

ISSUES:
  N. [SEVERITY: CRITICAL|HIGH|MEDIUM|LOW] [CATEGORY: INSTANCE_SHAPE|C_SEMANTICS|IMPL_LIFTING|SENTINEL|F4_2|FORWARD_FIT|CASCADE|FEASIBILITY|OTHER]
     LOCATION: <file and section, or "global">
     CONCERN: <one to four sentences>
     EVIDENCE: <citation supporting the concern>
     SUGGESTED FIX: <one to two sentences, or "none -- author's judgment">

(If no issues at all, write "ISSUES: none".)

CROSS_CHECK_NOTES: up to six bullets of claims you verified are
correct, to help the author trust the review.
```

Severity guide:

- **CRITICAL**: spec cannot be implemented as written, or
  implementing it would violate a settled architectural decision /
  prior-round resolution.
- **HIGH**: implementer would either build the wrong thing or stall
  on a substantive ambiguity that the spec must resolve.
- **MEDIUM**: real issue, but implementer can plausibly resolve it
  by re-reading methodology / research and making a defensible
  judgment call.
- **LOW**: stylistic or local clarity improvement.

Word cap: 2500 words total response (excluding citations).

## Default follow-through

Do not ask clarifying questions. The attachments contain everything
you need. If something is genuinely unclear, file it as an issue
under FEASIBILITY with a `HYPOTHESIS:` framing rather than asking.
Read the files, make calls, emit the structured response, stop.
