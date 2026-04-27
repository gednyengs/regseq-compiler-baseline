# Decision Log

Per-body lowering decisions for the pl011-full-spec fixture. The
canonical record: every non-trivial transformation from MIR to C is
captured here with its rationale and owning layer.

## Format (proposed; not yet finalized)

The current proposal for each entry is given below. Format is open for
revision based on the experience of the first two or three bodies.

Per-body entry skeleton:

```
## <body name>

**Source**: `path:line`, signature line.
**MIR fragment**: `pl011_full_spec.mir:Lstart-Lend`, ~N lines.
**Date lowered**: YYYY-MM-DD.

### Lowered C

(C output in fenced ```c block.)

### Decisions

For each non-uniform decision made during lowering:

1. **<one-line summary>**
   - Disposition: kept | dropped | rejected | transformed
   - MIR construct: `<the relevant snippet>`
   - Reason: <one or two lines explaining the choice>
   - Owning layer: MonoRust | RegseqModel | C backend | open
   - References: optional pointer to spec section, prior finding,
     or related decision.

### Open questions

(Anything the lowering surfaced that needs the user's call. Empty
section if none.)
```

Trivial uniform decisions (drop StorageLive, drop unwind annotations,
drop ZST writes, drop unreachable blocks following SwitchInt) do not
appear per-body. They live in the "Uniform rules" section below and
apply implicitly.

## Uniform rules

These rules apply to every body without per-entry justification. New
uniform rules are added here when an empirical finding generalizes
beyond a single body.

| Rule | Disposition | Owning layer | Source of rule |
|---|---|---|---|
| `StorageLive` / `StorageDead` statements | dropped silently | MonoRust extraction | C scopes encode lifetime |
| `unwind continue` annotation on Call/Drop | dropped silently | MonoRust extraction | spec is `panic = "abort"` |
| ZST writes (`_n = ()`, unit Continue extractions) | dropped silently | MonoRust extraction | no C representation |
| `bb_unreachable` after a 2-variant `SwitchInt` | dropped silently | MonoRust extraction | dead block by enum invariant |
| Reborrow expressions (`_5 = &(*_1)` for receivers) | transformed to direct param use | C backend | C passes pointers; reborrow is a Rust shape only |

## Reviewer prompt template

Adversarial review of a non-trivial decision is invoked via the
adversarial-reviewer agent (or Codex) using the prompt template below.
Substitute `<entry>` with the candidate entry's text.

```
<task>
Review the decision-log entry below. Verify each numbered decision
against the methodology document at docs/discussions/manual-lowering/methodology.md
and the inclusion-principle taxonomy in docs/discussions/monorust/overview.md
Section 6 (kept), 7.1 (gone by mono), 7.2 (rejected), 7.3 (invisible at MIR),
7.4 (dropped silently).

For each numbered decision, answer:
  1. Does the disposition (kept | dropped | rejected | transformed)
     match the inclusion-principle category cited?
  2. Does the owning layer attribution match the kind of decision
     being made (Rust-shape filtering -> MonoRust; domain recognition
     -> RegseqModel; C rendering -> C backend)?
  3. Does the entry hide any non-local analysis as a local decision?
     (For example: "lower this as a goto-chain cleanup" without
     calling out that this requires a dominator-based reverse-of-
     reversible-intrinsics analysis the compiler does not yet have.)

Entry under review:

<entry>
</task>
<grounding_rules>
Cite docs/discussions/manual-lowering/methodology.md and the
MonoRust spec for every flagged finding. Refer to a specific
numbered decision in the entry being reviewed when raising a
concern.
</grounding_rules>
<structured_output_contract>
First-line verdict: APPROVE or BLOCK.
Then a numbered list of findings, each with: which decision (number),
observation, severity (blocker | warning | note).
</structured_output_contract>
<default_follow_through_policy>
Read the documents, decide, stop. Do not ask clarifying questions.
</default_follow_through_policy>
```

## Entries

(No bodies lowered yet. The first entry will land here.)
