# Stage 1 Review Prompt

A self-contained prompt for an external reviewer (human or LLM) to
adversarially assess the Regseq stage-1 vocabulary specification.
The reviewer has no access to the project repository; everything
they need is in the attached files referenced below.

To use this: attach the files listed under "Attachments" alongside
this prompt and send to the reviewer.

---

## Role

You are conducting an adversarial pre-implementation review of a
specification document for an internal compiler IR. Your job is to
find issues that would cause an implementer to either build the
wrong thing or stall on ambiguity. You should be skeptical and
specific. Vague concerns are not useful; cite-and-fix concerns are.

You have no access to the project repository. Everything you need
is in the files attached with this prompt. References to files not
in the attachment set are out of scope -- if a document cites
"foo.md" and foo.md is not attached, do not flag the omission and
do not speculate about its contents.

## Project framing

Regseq is a Rust-to-C compiler for OS-agnostic device drivers.
Driver authors write `#![no_std]` Rust specs (with a `#[rseq::]`
attribute namespace and an embedded DSL); the compiler consumes
rustc's `optimized_mir` (with `mir_opt_level=0` and a curated
cleanup-pass whitelist) and produces C for target operating systems
(Linux, Zephyr).

The bridge between MIR and the C backend is an internal IR called
`RegseqModel`. The IR is specified bottom-up in 13 stages. This
review covers **stage 1: vocabulary** -- the foundational
table-and-id scaffolding that every later stage rides on. Stage 1
specifies five components:

1. ID types (item, body, type, const, block, local, span, string,
   source-file, instance).
2. The string interner.
3. The `SourceFile` table and `Span` representation.
4. The `DefPath` form (path segments with disambiguators) and
   monomorphization fingerprinting via `Instance`.
5. The top-level `RegseqModel` struct (which forward-declares
   tables that later stages populate).

`RegseqModel` is fully owned (no rustc lifetimes survive past the
bridge call), single-level (no IR ladder; stages refine the same
IR), and serializable (custom textual MIR-inspired pretty-printer
plus serde JSON).

## Attachments and how to read them

| File | Role |
|---|---|
| `stage-1-vocabulary.md` | The specification under review. Your primary target. |
| `stage-1-research.md` | The prior-art synthesis and the F1-F4 sub-decisions the spec instantiates. |
| `architecture.md` | Q1-Q7 architectural decisions the spec sits inside. SETTLED. |
| `memory.md` | Project briefing and methodology rules (the value system the spec must respect). |
| `regseq-background.md` | DSL background; what the source surface looks like. |
| `prior-art/README.md` | Synthesis of prior-art research that informed the architectural decisions. |
| `prior-art/rustc-public.md`, `prior-art/charon.md`, `prior-art/kani.md`, `prior-art/llvm-ir.md`, `prior-art/rustc-codegen-c.md` | Per-project prior-art files most heavily cited by the research synthesis. Use these to verify any "Charon does X" / "rustc_public does Y" claim before challenging it. |
| `rustc-excerpts.md` | Curated quotations from the pinned rustc-src checkout. Treat as authoritative for rustc-internal claims. |
| `stage-1-review-log.md` | Summary of two prior Codex review rounds and how each issue was addressed. Do not re-flag issues already resolved here. |

Read order if approaching cold: `memory.md` first (briefing +
methodology), then `regseq-background.md` (DSL), then
`architecture.md` (Q1-Q7), then `stage-1-research.md` (F1-F4 and
synthesis), then `stage-1-vocabulary.md` (the spec).
`stage-1-review-log.md` last, before drafting the review, so you
know what's already been considered.

## Out of scope

Do not flag any of the following:

- **Q1-Q7 architectural decisions** (in `architecture.md`). Settled.
  These constrain the spec; the spec is reviewed against them, but
  they are not subject to review.
- **F1-F4 sub-decisions** (in `stage-1-research.md`: span byte
  representation, span placement, source content, ItemId
  determinism). Settled. The spec instantiates them; only check that
  it does so correctly, not whether the underlying decisions are
  right.
- **Stage 3-13 IR shapes** (types, operands, statements, items,
  bodies, domain extensions). Out of stage-1 scope by design;
  stage 1 only forward-declares ID types, not the structures they
  point at.
- **Bridge translation logic** (the rustc-to-RegseqModel pass).
  Specified in a separate document not attached here.
- **Stylistic preferences** (naming, doc-comment phrasing). Focus
  on substance.
- **Issues already resolved in `stage-1-review-log.md`**. Re-flagging
  these wastes everyone's effort.

## Probe categories

Review across these six axes. For each, the question to ask of the
spec:

### 1. Type-shape consistency

Do the type sketches compose correctly? Is every ID kind that the
spec uses also defined? Do field types in one struct match the type
expected by another struct that holds it? Does the `Instance` shape
correctly reference `ItemId` (not `DefPathHash`), and does
`GenericArg` correctly reference `TyId` and `ConstId`? Is the
top-level `RegseqModel` struct consistent with the per-component
sections that precede it?

### 2. Coverage

Does the spec actually cover what later stages will need from
stage 1?

- Stage 3 (types) needs to put `TyId` entries somewhere -- is the
  table shape adequate?
- Stage 11 (bodies) needs `LocalDecl` indexed by `LocalId` and
  basic blocks indexed by `BlockId` -- does the body-local ID
  treatment make sense?
- Stage 12 (items) places `DefPath` on `Item` -- is the placement
  sensible? Does the consistency rule between `Item.def_path` and
  the side `item_def_paths` table hold?
- Stage 13 (domain extensions) adds new IDs (e.g., for register
  declarations or interrupt handlers) -- does the convention
  scale?
- Pretty-printer needs to render every IR construct -- does the
  `PrettyPrinter` sketch suffice?

### 3. F1-F4 instantiation correctness

For each of F1 (span byte representation), F2 (span placement), F3
(source content), F4 (ItemId determinism): does the spec actually
instantiate the locked decision? Cross-check against the relevant
section of `stage-1-research.md`. Mismatches between research and
spec are issues; mismatches between either of them and external
prior-art should be flagged in category 1 of the prior review log
or treated as out of scope.

### 4. Methodology violations

Per `memory.md`: no defensive fallbacks, compiler-theory-first,
core is opaque, source declares its own compiler-relevant
attributes (`register_tool`, `no_std`). Does the spec slip any
defensive fallbacks (e.g., panic-instead-of-reject, "if unknown
variant" branches that should not exist)? Does it conflict with
anything in `architecture.md` Q1-Q7?

### 5. Rustc-internal traps

The spec depends on rustc internals (the pinned-toolchain rustc-src
quoted in `rustc-excerpts.md`). Known traps include: `DefId` does
not implement `Ord`, `Symbol::as_str` returns a transmuted
reference, `BytePos` is a global offset across the SourceMap (not
file-local), `SymbolIndex` is not stable across runs. Does the
spec correctly account for these? Are there NEW rustc-internal
traps the spec creates? (Example to probe: the F4 sort comparator
runs against the rustc symbol interner during construction --
does the lifetime/borrow story hold?)

### 6. Implementation feasibility

Could a competent Rust engineer write the `regseq-model` crate
from this spec without surfacing major ambiguities? Where does the
spec under-specify something an implementer would need to invent?
For each ambiguity you find, note whether it is (a) deliberate
authorial-judgment latitude (acceptable) or (b) an oversight the
implementer cannot resolve from the spec alone (a real issue).

## Grounding rules

- **Cite file and section for every claim.** A concern without a
  cite is not actionable. The format `stage-1-vocabulary.md
  section "DefPath form"` is acceptable; line numbers are not
  required.
- **Treat `rustc-excerpts.md` as authoritative** for any
  rustc-internal claim. Do not invent rustc behavior.
- **Mark hypotheses as hypotheses.** Prefix any claim you cannot
  verify against an attached file with `HYPOTHESIS:`.
- **References outside the attachment set are out of scope.** If
  the spec cites `crates/regseq-model/` or some scripts/ path, do
  not chase those.
- **No issues found in a category is a valid finding.** Do not
  invent issues to fill probe categories.

## Output contract

Emit exactly this shape (no preamble, no postscript, no
restating-the-prompt):

```
VERDICT: <APPROVE_AS_IS | APPROVE_WITH_MINOR_REVISIONS | REVISE_BEFORE_IMPL | REJECT_AND_RETHINK>

ISSUES:
  N. [SEVERITY: CRITICAL|HIGH|MEDIUM|LOW] [CATEGORY: TYPE_SHAPE|COVERAGE|F1|F2|F3|F4|METHODOLOGY|TRAP|FEASIBILITY|OTHER]
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
  implementing it would violate a settled architectural decision.
- **HIGH**: implementer would either build the wrong thing or stall
  on a substantive ambiguity that the spec must resolve.
- **MEDIUM**: real issue, but implementer can plausibly resolve it
  by re-reading research or methodology docs and making a
  defensible judgment call.
- **LOW**: stylistic or local clarity improvement.

Word cap: 2500 words total response (excluding citations).

## Default follow-through

Do not ask clarifying questions. The attachments contain everything
you need. If something is genuinely unclear, file it as an issue
under FEASIBILITY with a `HYPOTHESIS:` framing rather than asking.
Read the files, make calls, emit the structured response, stop.
