# Compiler Collateral Package Review

## Purpose

This note records the final package-level review of the compiler-collateral
set after:

- overview and constraints validation
- validation of all 19 construct docs

The goal is to confirm that the package is coherent enough to hand to compiler
implementation work without forcing implementers back into the full research
tree for ordinary use.

## Review Scope

Files in scope:

- `README.md`
- `compiler-baseline-overview.md`
- `compiler-constraints.md`
- `constructs/README.md`
- all construct docs under `constructs/`

## Review Questions

1. Are there any stale package-level statements left?
2. Do overview, constraints, and construct docs contradict one another on any
   accepted baseline?
3. Is any cross-construct rule missing from the package entirely?
4. Is the package index/navigation good enough for implementation work?
5. Should the package be marked:
   - `accepted`
   - `accepted_with_small_patch`
   - `needs_revision`

## Initial Reviewer Notes

- stale package status text in `README.md` has been patched
- stale future-tense statement in `compiler-baseline-overview.md` has been
  patched
- construct-doc set is complete:
  - 19 construct docs listed in `constructs/README.md`
  - all 19 exist on disk
  - all 19 have passed construct-level validation

## Package-Level Review Findings

### Verdict: `accepted_with_small_patch`

The package is coherent and handoff-ready. One small additive patch and one
wording cleanup were worth applying:

- add an authoritative init-side ordering table to the overview
- fix stale `IC-1` through `IC-6` wording to `IC-1` through `IC-8`

With those applied, the package is handoff-ready. Two optional navigation
improvements remain noted below.

### 1. Construct-doc completeness

- constructs/README.md lists 19 construct docs (16 PL011/shared + 3 E1000)
- 19 .md files exist on disk under constructs/ (excluding README.md)
- all 19 match the listed set exactly; no missing docs, no stale entries
- all 19 have passed construct-level validation per the initial reviewer notes

### 2. Tone consistency

All 19 construct docs follow the implementer-oriented template consistently:

- Purpose sections explain constructs in compiler terms, not research terms
- No research-oriented language leaked through (no "Step 8 evidence," "concern
  [N]," or "elaboration notes" phrasing)
- Primitive inventories use concrete stable-surface columns
- Surface baselines include Rust code where applicable
- Extraction baselines list what the frontend must recognize
- Lowering baselines specify ownership and lifecycle-phase participation
- Constraints and implementation checklists are actionable

No construct doc reads as research-oriented.

### 3. Cross-doc consistency

Spot-checked boundaries across validated construct docs:

- Atomic vs Register Access: MMIO barrier/fence boundary clean; both docs
  independently state the no-io_fence exclusion
- Synchronization vs Atomic: lock/completion vs typed shared-state boundary
  clean; RegseqError vs Result<T,T> distinction preserved
- Serial vs Driver Lifecycle: device_class selector, nested port_open/close
  lifecycle, and separate register_driver primitives all consistent
- Interrupt and Timer: bound-handler model consistent (callback entry
  context H, no-auto-lock, method_attribute surface form)
- PM ordering: constructs that participate in PM phases (Clock, PinCtrl,
  Interrupt) cite the same S-1/S-2/S-3 and R-1/R-2/R-3 positions
- cpu_relax exclusion from timed-delay surface is stated in both timed-delay
  and register-access docs

No cross-doc contradictions found.

### 4. Package navigation

Current navigation path:

- compiler-baseline-overview.md: top-level accepted surface families, cross-
  construct baselines, construct index pointer
- compiler-constraints.md: shared implementation constraints (IC-1 through IC-8
  plus caveat families)
- constructs/README.md: section template and construct list
- constructs/*.md: per-construct docs

This is adequate for implementation work. Two optional improvements:

**Optional improvement A: construct-shape grouping in constructs/README.md.**

The current list is ordered roughly by mapping sequence. An implementor would
benefit from a grouping by implementation shape, since different shapes require
different compiler pipeline stages:

- trait callback constructs (trait extraction/lowering):
  driver-lifecycle, device-power-state-management, serial-uart-management,
  network-interface-management
- bound-handler constructs:
  device-interrupt-setup-and-handling, kernel-timer-lifecycle-management,
  deferred-work-item-scheduling
- Typed-resource wrapper constructs:
  register-access, peripheral-clock-management, hardware-reset-sequencing-
  and-post-reset-polling, pin-controller-and-gpio-configuration,
  synchronization, atomic-memory-access-and-ordering, dma-management
- Intrinsic-only constructs:
  timed-cpu-delay-and-sleep, diagnostic-logging
- Declarative/compiler-managed-only constructs:
  firmware-property-access, bus-resource-management, parameter-registration

This grouping maps directly to compiler pipeline stages: trait extraction,
bound-handler extraction, wrapper recognition, intrinsic recognition,
attribute/declarative extraction.

**Optional improvement B: compact cross-construct reference matrix.**

A small table in compiler-baseline-overview.md or constructs/README.md showing
which constructs participate in which cross-cutting concerns (PM phases, Track
A/B, callback-context class, Duration consumers, Layer 2 binding) would help
implementors navigate without reading every doc. This is additive polish, not a
gap.

### 5. Handoff readiness

The package is ready for compiler implementation handoff:

- all 19 construct docs are present, validated, and implementer-oriented
- overview and constraints docs are current and non-contradictory
- no cross-doc inconsistencies found
- no missing cross-construct rules identified
- the two optional improvements above are additive polish, not blockers

---

## researcher-1 Independent Review

### Staleness check

No stale package-level statements found in overview or constraints. Prior
validation feedback (SerialConsole split, &mut self convention, Work in
the bound-handler surface, trait name wording, MMIO barrier reference
cleanup, firmware
PROP check reference, vendor_prefix note, DMA IC-3 detail, timer
auto_routing signals, PM phase-table clarification) has all been
incorporated.

One stale reference in this review file itself: Section 4 previously said
"IC-1 through IC-6" but `compiler-constraints.md` now has IC-1 through
IC-8 (IC-7: SoC power-domain ordering; IC-8: poll-loop yield hints).

Overview scope claim "P1-01 through P1-10" is accurate: P1-01 through
P1-08 have explicit cross-construct baselines sections; P1-09 is evidence
hygiene (no baseline content needed); P1-10 results are covered in the
E1000 Extension Surface section.

### Contradiction check

No contradictions found between overview and constraints:

- DMA callback exclusion consistent across overview and constraints (IC-3,
  Class C).
- PM ordering consistent: overview states the generated Class G sequence;
  IC-7 adds power-domain as a backend step without contradicting it.
- Poll lowering consistent: overview states `regseq::poll::until(...)` as
  surface; IC-8 handles yield hints as backend obligation.
- SerialConsole references consistent across overview and IC-2/IC-4.
- Receiver conventions correctly split: `&mut self` for trait callbacks,
  `&self` for wrapper methods.

### Missing cross-construct rule: init-preamble ordering (MEDIUM)

The package has no single authoritative init-preamble ordering table.
Individual construct docs state their own phase participation. The PM doc
has the full suspend/resume sequence. But there is no equivalent single
reference for the init preamble ordering.

An implementer would need to piece together the init ordering from
multiple construct docs. The accepted ordering based on construct-doc
fragments and v2.2 precedent is:

| Step | Construct | Action |
|------|-----------|--------|
| 1 | Driver Lifecycle | create_device + associate_private_data |
| 2 | Bus Resource Management | get_resource_handle |
| 3 | Register Access | map_region (MMIO) |
| 4 | Clock Management | acquire_clock + enable |
| 5 | Hardware Reset | acquire_reset + deassert |
| 6 | PinCtrl | acquire_pinctrl + apply default |
| 7 | Firmware Property Access | read_property |
| 8 | Timer | init_timer |
| 9 | DMA | alloc_coherent + request_channel |
| 10 | Interrupt | register_handler (not yet enabled) |
| 11 | Driver Lifecycle | Lifecycle::init() body runs |
| 12 | Interrupt | enable (compiler-managed postamble) |

Deinit postamble is roughly the reverse.

Resolved: an "Init-side generated ordering reference" subsection has been
added to `compiler-baseline-overview.md`.

### Minor inconsistency: P1-02 cross-reference scope (LOW)

Overview cross-construct baselines section says "P1-02: shared bound-handler
model for Interrupt and Timer." Section 3 of the same document correctly
says "used for Interrupt, Timer, and Work handler-shaped behavior." P1-02
itself covers only Interrupt and Timer; Work was added in P1-10.

Recommended action: add parenthetical to the P1-02 line: "shared bound-handler
model for Interrupt and Timer (extended to Work in P1-10)."

### Verdict alignment

The reviewer's `accepted` verdict is correct. The init-preamble ordering
gap is the only item that would send an implementer back to the research
tree, and it was bounded enough to address as a small additive patch rather
than a revision. The P1-02 wording is cosmetic.

---

## researcher-3 Independent Review

### Cross-package consistency: five dimensions checked

Systematic verification across all 19 construct docs, the overview, and the
constraints doc. Each dimension checked for contradictions.

#### Dimension 1: Callback model -- CONSISTENT

Trait callbacks: Lifecycle (F, &mut self, RegseqError), PM (F, &mut
self, RegseqError), Serial (S, &mut self, RegseqError + domain), Network (S,
&mut self, RegseqError + domain). All match overview lines 67-89.

Bound handlers: Interrupt (H, IrqResult), Timer (H, void), Work (H, void).
All three confirm no-auto-lock. All match overview lines 91-100.

Non-callback constructs: all wrapper families use &self. Free-function
intrinsics and macros have no receiver. Declarative/compiler-managed
constructs have no wrapper surface. Matches overview line 108.

#### Dimension 2: Bound handlers -- CONSISTENT

Overview line 95 lists Interrupt, Timer, and Work. All three construct docs
confirm H class and no-auto-lock. Synchronization doc constrains all three.
No other construct doc claims the bound-handler surface. PinCtrl and Bus
Resource explicitly state they do not use it.

#### Dimension 3: PM ordering and ownership -- CONSISTENT

Class G (compiler-generated): Clock, PinCtrl, Interrupt. Each construct doc
correctly delegates PM-phase behavior to the PM construct.

Class V (driver-authored/compiler-validated): Timer, DMA. Both state class V
and "not in generated sequence."

All remaining constructs: no PM claims or correctly not PM-relevant.

Interrupt lifecycle table: enable in init+resume, disable in suspend+deinit.
Matches PM ordering positions S-1 and R-3.

Register Access: map_region in init/deinit only, not suspend/resume. Correct.

IC-7 (SoC power-domain ordering) is consistent with PM generated sequence.

#### Dimension 4: Wrapper conventions -- CONSISTENT

All 9 wrapper constructs (Register Access, Clock, PinCtrl, ResetLine, Sync,
Timer, DMA, Atomic, Work) use &self. Overview line 108 confirms.

#### Dimension 5: Audit constraint carry-through -- CONSISTENT

IC-1 through IC-8 verified. Construct docs reference constraints by
description, not by IC number, matching the constraints doc guidance (lines
136-137). Each IC is either referenced in the relevant construct doc or
covered adequately by the shared constraints doc alone. No IC is lost.

### Agreement with researcher-1 findings

I confirm both findings:

1. **Init-preamble ordering gap (MEDIUM)**: Agree. The package has explicit
   suspend/resume ordering (PM doc) but no single init-preamble ordering
   table. Researcher-1's proposed 12-step table is correct and matches the
   construct-doc lifecycle phase tables when assembled. This is the only
   item that would force an implementer to piece together ordering from
   multiple docs.

2. **P1-02 cross-reference scope (LOW)**: Agree. The parenthetical fix
   "(extended to Work in P1-10)" is sufficient.

### No additional issues found

My five-dimension check found no contradictions beyond what the initial
reviewer and researcher-1 already identified. The stale IC-6 reference on
line 101 was already caught by researcher-1.

### Verdict

**accepted**

The package is ready for compiler implementation handoff. The init-preamble
ordering table is the one additive item worth patching before handoff, but
it is bounded (one table in one file) and does not change any accepted
decision.
