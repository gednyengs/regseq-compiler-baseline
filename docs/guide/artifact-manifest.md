# Artifact Manifest

Source artifacts accepted for Track A. Each artifact is frozen and
should not be modified by compiler/DSL work.

## Step 7: Construct Grouping

**`step-7-construct-grouping/output/artifact_constructs.json`**
Approved construct grouping: 54 non-language concerns grouped into
9 multi-concern constructs, 16 singletons, 8 utilities. Provides
concern-to-construct mapping and grouping rationale.
Compiler use: construct identity, member concerns, grouping lineage.

**`step-7-construct-grouping/output/artifact_dsl_surface.md`**
Minimal DSL surface definition: 6 new constructs, 1 extension, resource
types, intrinsics, traits, attributes, and example driver structs for
PL011 and E1000. The DSL surface is the syntactical contract for
compiler work.
Compiler use: resource type APIs, attribute schemas, intrinsic
signatures, platform file format.

## Step 8: Construct Synthesis

**`step-8-construct-synthesis/output/artifact_synthesized_constructs.json`**
Normalized Step 8b output: 9 constructs + 11 singletons + 5 deferred.
102 operations (54 trait, 18 intrinsic, 30 compiler-managed), 70
declarable properties with ownership annotations, cross-OS mechanism
comparisons, Step 9 risk notes.
Compiler use: operation signatures, property schemas, per-OS lowering
targets, operation classes.

**`step-8-construct-synthesis/output/artifact_property_evidence.json`**
70 property evidence entries with 100% coverage. Each declarable
property has source concerns, source element IDs, Step 3 evidence,
and rationale for why it is declarable.
Compiler use: traceability; understanding why a property exists.

**`step-8-construct-synthesis/output/artifact_synthesis_decision_log.json`**
45 synthesis decisions: operation splits/collapses, property
classifications, reclassifications, ownership resolutions, Step 9
risk flags.
Compiler use: understanding boundary decisions and design rationale.

## Step 9: Construct Validation

**`step-9-construct-validation/output/artifact_validated_constructs.json`**
Final validated construct inventory: 19 active constructs (13 accepted,
6 accepted_with_caution), 1 rejected (Capability Extraction, subsumed
by Register Access), 5 deferred singletons. Each construct has 6-check
pass/fail evidence and validation status.
Compiler use: authoritative construct list; caution notes for
implementation.

**`step-9-construct-validation/output/artifact_capture_map.json`**
Per-element mapping for 2,787 PL011/E1000 elements. Each element
assigned to exactly one category with zero gaps.
Compiler use: verifying that implementation handles all artifact
elements; understanding how source code maps to DSL constructs.

**`step-9-construct-validation/output/artifact_validation_decision_log.json`**
4 validation decisions: methodology rules, check interpretations,
rejection rationale.
Compiler use: understanding why Capability Extraction was rejected;
interpretive notes for property-light constructs.

## Supporting artifacts

**`step-6-concern-validation/output/validated_inventory.json`**
Frozen artifact-slice inventory: 71 concerns, 15,397 elements. Provides
per-concern mechanisms_by_os, element IDs, and OS/device counts.

**`step-4-concern-identification/output/elements.json`**
15,744 elements with per-element metadata. Join key for tracing
elements back to Step 3 work packages.

**`step-3-intent-description/work-packages/{os}/*.json`**
Full Step 3 work package context: descriptions, specification_decisions,
parts[N].role. Authoritative evidence source for property synthesis.
