# PL011 Full Spec Discussion

This directory is the working area for defining the full PL011 Regseq spec we
intend to carry through the compiler pipeline and, ultimately, the research
evaluation flow.

For this version of the full PL011 target, we are intentionally pinning the
spec to a single concrete ARM PL011 configuration. We are not trying to model
family-wide variant selection, revision dispatch, or quirk/erratum machinery in
this version of the spec.

For the same reason, we are also not forcing variant-specific PM behavior into
the current target. The corpus evidence shows that meaningful PM participation
for PL011 is variant-driven rather than part of the generic `arm,pl011`
frontend.

Use this discussion area to:

- enumerate the full accepted PL011 construct set
- define the intended Layer 1 PL011 spec surface
- define the intended Layer 2 binding/platform shape for PL011
- identify what must be completed in Phase B before Phase C model work
- record explicit deferrals, if any, even for the "full PL011" target

This discussion should drive:

- any reopened Phase B PL011 expansion batches
- the final Phase B closure target
- the Phase C normalized-model scope
