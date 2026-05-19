# E1000 Full Spec Methodology

> **⚠ PARTIALLY SUPERSEDED (2026-05-19).** The evaluation-strategy framing
> here (axes-derived claim shape, QEMU-gated staging) is superseded by
> `../e1000-evaluation-strategy/`. The per-feature *spec-growth* workflow
> (§3–§7: evidence-driven fixture growth, compile-check discipline) is
> retained, but must align with the minimal-sufficient strategy — it no
> longer feeds a full axis matrix or a staged QEMU-patch plan.

**Status**: PARTIALLY SUPERSEDED (was SKELETON). See banner above.

**Role**: documents the methodology by which the full-E1000 Regseq spec
target is derived, grown, and compile-checked. Parallels
`pl011-full-spec-methodology.md` but starts directly from the axes-derived
methodology rather than the older feature-inventory approach (PL011's
methodology was reframed to axis-derived on 2026-04-20; E1000 starts there).

## 1. Methodology summary (to be authored)

The working methodology for E1000 is:

- **comparison axes**, derived from concrete E1000 system-use cases, decide
  what goes in the spec
- **corpus drivers** (Linux e1000, FreeBSD em legacy code paths, ReactOS
  e1000, Zephyr eth_e1000) are baselines for comparison and policy sources
  for convergent best-practice (e.g., RX interrupt moderation defaults,
  descriptor-ring sizing, link-up handshake), NOT inclusion gates
- the spec is evidence-driven along three requirement-sourcing classes
  (axis / correctness / backend-contract) per
  `e1000-spec-requirements.md`

Authoritative artifacts:

- `e1000-comparison-axes.md`
- `e1000-workloads.md`
- `e1000-spec-requirements.md`

## 2. Scope boundary

This version targets a single concrete Intel 82540EM E1000 configuration.
Family-wide variant/revision/quirk machinery is intentionally deferred. The
e1000e PCIe family is out of scope by the variant pin.

See `README.md` for the precise scope pin.

## 3. Workflow (to be authored)

Per-feature workflow mirroring the PL011 methodology:

1. Start with an empty or near-empty `tests/fixtures/e1000-full-spec/` crate.
2. Choose the next feature family from the agreed full-E1000 target using
   the comparison axes and requirements as the scope source of truth.
3. For that feature family only, inspect existing driver sources where the
   feature is implemented and extract the hardware/driver intent that must
   be represented in Regseq.
4. Do not trace entire OS drivers end to end and do not mirror framework
   glue, allocation/setup scaffolding, or backend-only control flow into
   the Regseq spec.
5. Add the feature to the fixture in Regseq form.
6. After each change, compile the fixture through the real `regseq-driver`
   wrapper path.
7. If the feature compiles and extracts cleanly, keep it and continue.
8. If the feature exposes a missing frontend capability:
   - decide whether it is already accepted in the current taxonomy
   - if yes, record it as required reopened expansion work
   - if no, stop and resolve the design first
9. Repeat until the intended fuller E1000 spec target is concretely
   expressed in the fixture.

## 4. Per-feature review loop (to be authored)

For each feature family, use the same four-step loop:

1. Inspect the Linux, FreeBSD, ReactOS, and Zephyr corpora for that feature
   only.
2. Synthesize the common hardware/driver intent and the Layer 1 / Layer 2 /
   compiler split before changing the fixture.
3. Add only that feature to `e1000-full-spec` and compile-check it through
   the real `regseq-driver` wrapper path.
4. Judge whether the resulting Regseq frontend can lower/match the target
   OS correctly under Regseq's layering rules.

## 5. Proposed feature-addition order (to be authored)

To be derived from the comparison axes once authored. Provisional sketch:

1. Base declarations
   - `E1000Registers`
   - `Mmio<E1000Registers>`
   - core properties (mac-address, max-frame-size, ring-sizes)
   - `Interrupt`
2. Receive-path bring-up (RX descriptor ring, RAR/MTA filtering, RDH/RDT)
3. Transmit-path bring-up (TX descriptor ring, TDH/TDT, TIPG)
4. Interrupt-driven service (`ICR`/`ICS`/`IMS`/`IMC`, RX/TX/link IRQ sources)
5. Link management and PHY interaction (`STATUS`, `CTRL`, MDIO via `MDIC`)
6. Hardware checksum offload (TX/RX)
7. Interrupt moderation (`RDTR`, `RADV`, `ITR`)
8. Multicast hash filter / promiscuous control
9. EEPROM read (`EERD`)
10. Optional richer features (basic VLAN, WoL)

This order is provisional. The authoritative order will follow from the
axes and stage-tagging in `e1000-spec-requirements.md` Section 1.2.

Explicit deferrals for this version:

- e1000e family and PCIe-only features
- TSO and large-segment offload
- MSI-X and multi-queue
- advanced RSS
- SR-IOV / VF support
- (additional deferrals to be enumerated)

## 6. Important discipline (to be authored)

Parallels PL011 Section "Important discipline":

Every time a new feature reveals a gap, record one of:

- an accepted gap that should become reopened expansion work
- an explicit deferral
- a design question that must be resolved before continuing

The full-spec fixture should therefore be evidence-driven, not an
uncontrolled accumulation of partial ideas.

## 7. Relationship to the compiler pipeline (to be authored)

This methodology is frontend-first. It is intended to stabilize:

- the fuller E1000 Layer 1 surface
- the fuller E1000 Layer 2 expectations
- the construct set that the declaration model must absorb to handle
  network-class devices in addition to serial-class devices

It is NOT intended for normalized model design, Layer 1 + Layer 2 join
logic, lowering, or backend realization.

## Change history

- 2026-05-04: skeleton stood up. Authoritative content awaits authoring.
- 2026-05-19: Evaluation-strategy framing superseded by
  `../e1000-evaluation-strategy/`; per-feature spec-growth workflow retained
  and re-scoped to the minimal-sufficient strategy.
