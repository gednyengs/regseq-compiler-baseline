# Construct Compiler Docs

This directory holds one compiler-facing document per accepted construct.

These docs are derived from:

- `../regseq-primitive-mapping-framework.md`
- `../construct-elaborations/`
- `../pass1-notes/`
- `../full-device-audit/`

## Purpose

Each construct doc should let a compiler implementer answer:

- what primitives belong to this construct
- how they appear in the accepted surface
- how extraction finds them
- who lowers them
- what dependencies and constraints matter

Some constructs may also have narrower device-class refinement docs under
`docs/discussions/` when later compiler-facing work needs a more precise
contract than the broad construct baseline. When such a refinement is marked
authoritative for implementation, keep the construct doc as the broad accepted
scope reference and implement from the refinement where they differ.

## Recommended Section Template

Use this shape for each construct document:

1. `Purpose`
   - what the construct means in compiler terms
2. `Accepted Primitive Inventory`
   - compact list of accepted primitives
   - include the Layer 1 vs Layer 2 split for construct properties/resources
3. `Accepted Surface Baseline`
   - trait / handler / wrapper / intrinsic / compiler-managed split
   - for callback-bearing constructs, include callback entry context:
     - `F`: framework-managed entry
     - `S`: subsystem-serialized entry
     - `H`: explicit handler-managed entry
4. `Extraction Baseline`
   - concrete source-level patterns the frontend must recognize
   - dominant semantic carrier / primary specification signal where relevant
5. `Lowering Baseline`
   - lowering owner and generated lifecycle participation
   - include a generated lifecycle phase table:
     - init preamble
     - deinit postamble
     - suspend postamble
     - resume preamble
6. `Cross-Construct Dependencies`
   - what this construct depends on and who depends on it
7. `Constraints and Caveats`
   - only the construct-relevant subset of shared constraints
8. `Representative Examples`
   - small PL011 and/or E1000 examples where useful
9. `Implementation Checklist`
   - the concrete things the compiler work for this construct must support

## Initial Construct Set

PL011 / shared constructs:

- driver-lifecycle.md
- device-power-state-management.md
- serial-uart-management.md
- register-access.md
- peripheral-clock-management.md
- firmware-property-access.md
- device-interrupt-setup-and-handling.md
- synchronization.md
- dma-management.md
- kernel-timer-lifecycle-management.md
- timed-cpu-delay-and-sleep.md
- diagnostic-logging.md
- atomic-memory-access-and-ordering.md
- pin-controller-and-gpio-configuration.md
- bus-resource-management.md
- hardware-reset-sequencing-and-post-reset-polling.md

E1000 extensions:

- network-interface-management.md
- parameter-registration.md
- deferred-work-item-scheduling.md
