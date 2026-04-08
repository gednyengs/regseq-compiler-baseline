# PL011 Extraction Checks

This directory contains the source-to-extraction check contracts for the PL011
full-spec fixture. It is also the cold-start entry point for agents that need
to continue the extraction-remediation workflow.

Use PL011 as the proving fixture, not as the implementation dispatch key. Any
compiler fix produced from these checks must generalize to structurally similar
drivers and must not introduce PL011-specific or OS-specific hacks.

## Directory Layout

- `docs/validation/01-pl011/checks/<NN>-*.md`
  The extraction check contract for one authored->extraction boundary.
- `docs/validation/01-pl011/checks/results/<NN>-*.md`
  The reviewed validation certificate for that check.
- `docs/validation/01-pl011/checks/remediation/<NN>-*.md`
  The remediation backlog/checklist derived from the reviewed certificate.

## Cold-Start Workflow For Remediation

If you are picking up a new remediation task cold, use this workflow.

### 1. Read the check contract

Start with the check file:

- `docs/validation/01-pl011/checks/<NN>-*.md`

This tells you:

- what authored facts must survive
- what extraction surface is in scope
- what proof strength is required

### 2. Read the reviewed certificate

Then read the reviewed result:

- `docs/validation/01-pl011/checks/results/<NN>-*.md`

This tells you:

- what currently passes
- what still fails
- what exact evidence the existing validation test was able to prove
- what the actual gap is at the current extraction boundary

### 3. Read the remediation checklist

Then read the corresponding remediation file:

- `docs/validation/01-pl011/checks/remediation/<NN>-*.md`

This tells you:

- whether the check still needs implementation work
- the current extraction gaps
- likely implementation touchpoints
- the minimum facts that need to be preserved

If the remediation file says no work is currently required, treat that check as
closed unless a new regression has appeared.

### 4. Read a few completed remediation task specs

Before authoring a new remediation task spec, read a few existing harness task
specs to learn the expected format and level of precision.

Recommended examples:

- `.harness/tasks/extraction-remediation/01-pl011-register-block-extraction-remediation.md`
- `.harness/tasks/extraction-remediation/04-pl011-utility-functions-extraction-remediation.md`
- `.harness/tasks/extraction-remediation/07-pl011-helper-methods-a-extraction-remediation.md`
- `.harness/tasks/extraction-remediation/08-pl011-helper-methods-b-extraction-remediation.md`

These show the expected sections:

- description
- input context
- output specification
- constraints
- implementation notes
- test plan
- acceptance criteria

They also show the discipline we want:

- keep implementation guidance general
- move PL011-specific proof targets into the test plan
- explicitly forbid PL011-specific compiler logic
- require honest report updates if the implementation is incomplete

### 5. Create the remediation harness task spec

Create a new harness task spec under:

- `.harness/tasks/extraction-remediation/`

Then register it in:

- `.harness/tasks.json`

The task spec should point at:

- the check contract
- the reviewed certificate
- the remediation checklist
- the task-owned validation test

It should also say exactly:

- what code surfaces may be modified
- what test/report artifacts must be updated
- what constraints the implementer must follow
- how to rerun the task-owned validation test deterministically

## Required Constraints For Remediation Tasks

Every new extraction-remediation task spec should make these constraints
explicit.

- Do not weaken the extraction check contract.
- Do not change authored-source facts to make the task easier.
- Do not edit unrelated `source-to-extraction-*` test files or reports.
- Use PL011 as the validation target, not as the implementation dispatch key.
- Do not add PL011-specific branches, helper-name pattern matches, or
  fixture-path special cases in the compiler.
- Any extraction improvement must generalize to structurally similar authored
  code outside the PL011 fixture.
- Do not add OS-specific branches or backend-specific shortcuts to satisfy an
  extraction contract. The compiler must remain usable across different target
  OS backends.

## Post-Implementation Verification Workflow

Once the remediation implementation is complete, a separate verification pass
should check both correctness and design quality.

Before starting verification, the verifying agent must create an explicit
numbered verification checklist and show it first.

The agent must track progress against that checklist explicitly and must not
finish until every checklist item has a final disposition:

- `complete`
- `blocked` with a concrete reason

A verification response is incomplete if it does not include:

- the checklist itself
- a disposition for every checklist item

Required verification checklist:

1. Verify the implementation is correct.
2. Validate that the right things were implemented.
3. Verify that no PL011-specific hacks were used.
4. Verify that no OS-specific hacks were used.
5. Flag any fragile implementation strategies that avoid the correct general
   solution in favor of narrow escapes that only solve the current fixture.
6. Make sure all tests pass and there are no warnings.
7. Verify that no adjacent passing extraction checks regressed.
8. Verify that the fix changed the compiler boundary itself, not only the
   task-owned test.
9. Verify that the new or changed representation is internally coherent across
   all producers and consumers.
10. Verify that no dead, duplicated, or conflicting representations remain.
11. Verify that the fix does not invent unauthorized facts that were not
    authored upstream.
12. Verify that the new representation is actually useful to downstream
    compiler phases, rather than being inert proof-only data.
13. Assess whether there is a better-established strategy for solving the same
    compiler problem, based on common compiler practice, prior art, or
    established Rust idioms, and note any recommended alternative.

### Verification Inputs

The verifying agent should read:

- the check contract
- the reviewed certificate before the remediation
- the remediation checklist
- the remediation task spec
- the implementation diff
- the updated task-owned validation test
- the updated validation report

### Verification Expectations

The verifying agent must answer all of these:

- Is the implementation technically correct for the boundary?
- Does it preserve the exact missing facts the check required?
- Did it solve the gap in the compiler, rather than merely weaken the proof?
- Did it keep the compiler general across devices?
- Did it keep the compiler general across OS backends?
- Did it introduce fragile modeling or extraction shortcuts that should be
  flagged even if the current fixture passes?
- Did any nearby extraction boundary that previously passed now regress?
- Does the changed representation mean the same thing everywhere it is
  produced and consumed?
- Did the fix leave dead or duplicated facts behind?
- Does the fix preserve upstream truth without inventing new unsupported facts?
- Can at least one downstream phase use the new information coherently?
- Is there a better-established strategy for this kind of compiler problem in
  common compiler practice, prior art, or established Rust idioms, and if so,
  should it be adopted now or recorded as follow-up?

### Minimum Verification Commands

At minimum, rerun:

- the task-owned `source_to_extraction_*` test for the remediated check
- any focused tests needed to validate the touched extraction/model surfaces
- any adjacent `source-to-extraction-*` tests that exercise the same changed
  compiler surfaces
- `cargo test --workspace`

Verification should also ensure there are no warnings after the harness cleanup
and final checks.

## Design Standard

The extraction pipeline should become more general and more explicit over time.
Do not accept these as “fixes”:

- fixture-name dispatch
- helper-name dispatch
- hardcoded PL011 constant/path recognition in production extraction logic
- backend- or OS-specific branching inside extraction just to satisfy one check
- proof-only test tricks that compensate for weak extraction instead of fixing
  the underlying compiler representation

Prefer these instead:

- stronger extracted data structures
- better MIR/HIR/body modeling
- explicit operand and control-flow facts
- reusable representations that downstream compiler phases can consume
