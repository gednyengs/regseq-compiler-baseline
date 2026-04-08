# Validation Certificate: PL011 Pin-State Markers Extraction Check

Source premise: `docs/validation/01-pl011/01-authored-source.md`
Target boundary evidence: `crates/regseq-driver/tests/pin_state_markers_extraction.rs`

## Specimen Fidelity

A reduced inline specimen is used. Specimen fidelity is established by the
`precondition_specimen_fidelity` test, which reads the real fixture source at
`tests/fixtures/pl011-full-spec/src/driver.rs` and asserts that every authored
fact in scope appears verbatim in both the fixture and the specimen.

Fidelity Assertion Predicate(s):

- `fixture.contains("pub struct DefaultState;")` -- zero-sized marker type
- `fixture.contains("pub struct SleepState;")` -- zero-sized marker type
- `fixture.contains("impl PinState for DefaultState {}")` -- trait impl
- `fixture.contains("impl PinState for SleepState {}")` -- trait impl
- `fixture.contains("pinctrl.apply::<DefaultState>()")` -- usage site
- `fixture.contains("pinctrl.apply::<SleepState>()")` -- usage site
- `fixture.contains("fn apply_default_pins_if_present")` -- containing fn
- `fixture.contains("fn apply_sleep_pins_if_present")` -- containing fn
- Same eight assertions applied to `SPECIMEN` constant

Fidelity Evidence (Exact Citations):

- Test `precondition_specimen_fidelity` in
  `crates/regseq-driver/tests/pin_state_markers_extraction.rs`, lines 214-303.
- All 16 assertions pass (8 for fixture, 8 for specimen).

## Check-to-Surface Mapping

| Check | Extraction Surface | API |
|---|---|---|
| 1a (distinct structs) | `IdentityEnrichmentSurface.structs` | `extract_identity_enrichment(tcx)` |
| 1b (no collapse) | `IdentityEnrichmentSurface.structs` | `extract_identity_enrichment(tcx)` |
| 2a (trait impl evidence) | `IdentityEnrichmentSurface.impl_blocks` | `extract_identity_enrichment(tcx)` |
| 2b (usage-site distinction) | `PinCtrlSurface.call_sites` | `extract_pinctrl_surface(tcx)` |
| 2c (state_type distinctness) | `PinCtrlSurface.call_sites` | `extract_pinctrl_surface(tcx)` |

## Check Group 1: Marker-type identity remains distinct

### Check 1a: Both marker types are extracted as distinct structs

- Check: DefaultState and SleepState are extracted as distinct structs with
  distinct canonical paths in the identity enrichment surface.
- Status: `pass`
- Premise: Both marker types remain identifiable and distinct.
- Assertion Predicate(s):
  - `result.identity.structs.iter().find(|s| s.name == "DefaultState").is_some()`
  - `result.identity.structs.iter().find(|s| s.name == "SleepState").is_some()`
  - `default_state.canonical_path != sleep_state.canonical_path`
  - `default_state.canonical_path.0.contains("DefaultState")`
  - `sleep_state.canonical_path.0.contains("SleepState")`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check1a_marker_types_are_distinct_structs` in
    `crates/regseq-driver/tests/pin_state_markers_extraction.rs`, lines 312-356.
  - Both structs found in `IdentityEnrichmentSurface.structs`.
  - Canonical paths are distinct and contain the respective type names.
- Reasoning Trace:
  - Authored Fact (Verbatim): `pub struct DefaultState;` and `pub struct SleepState;`
    as zero-sized marker types with canonical paths
    `pl011_full_spec::driver::DefaultState` and `pl011_full_spec::driver::SleepState`.
  - Extracted Fact (Verbatim): `StructFact { name: "DefaultState", canonical_path: ... }`
    and `StructFact { name: "SleepState", canonical_path: ... }` both present.
  - Correspondence: `exact`
  - Gap / Loss Description: None.
  - Why This Status Follows: The identity enrichment extraction preserves both
    marker types as distinct structs with distinct canonical paths. The assertions
    directly inspect the extraction surface and would fail if either type were
    missing or if they shared the same canonical path.
- Counterexample: N/A
- Conclusion: Pass. Marker-type identity is preserved as distinct structs.

### Check 1b: No marker collapse (closed-world)

- Check: The complete set of structs named DefaultState or SleepState contains
  exactly one of each.
- Status: `pass`
- Premise: DefaultState and SleepState do not collapse.
- Assertion Predicate(s):
  - `result.identity.structs.iter().filter(|s| s.name == "DefaultState").count() == 1`
  - `result.identity.structs.iter().filter(|s| s.name == "SleepState").count() == 1`
  - `marker_names.len() == 2` (closed-world: exactly two pin-state markers total)
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check1b_no_marker_collapse` in
    `crates/regseq-driver/tests/pin_state_markers_extraction.rs`, lines 361-402.
  - Counts over the complete `structs` vector confirm exactly one DefaultState
    and exactly one SleepState. Total marker count is 2.
- Reasoning Trace:
  - Authored Fact (Verbatim): Two distinct marker types: DefaultState and SleepState.
  - Extracted Fact (Verbatim): Exactly one struct named DefaultState and exactly one
    struct named SleepState in the complete identity enrichment struct list.
  - Correspondence: `exact`
  - Gap / Loss Description: None.
  - Why This Status Follows: Closed-world enumeration over the full extracted struct
    set confirms no duplication and no loss. The assertions would fail if either
    type were missing, duplicated, or collapsed into the other.
- Counterexample: N/A
- Conclusion: Pass. No marker collapse detected.

## Check Group 2: PinState implementation evidence survives

### Check 2a: Trait impl blocks for PinState exist for both markers

- Check: Trait impl blocks where trait_name is "PinState" exist for both
  DefaultState and SleepState in the identity enrichment surface.
- Status: `pass`
- Premise: PinState implementation evidence survives for each marker.
- Assertion Predicate(s):
  - `pinstate_impls.len() == 2` (exactly two PinState trait impls)
  - `impl_self_types.contains(&"DefaultState")`
  - `impl_self_types.contains(&"SleepState")`
  - `!ib.is_inherent` for each PinState impl (they are trait impls, not inherent)
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2a_pinstate_trait_impl_evidence` in
    `crates/regseq-driver/tests/pin_state_markers_extraction.rs`, lines 411-459.
  - Filtered `impl_blocks` by `trait_name == Some("PinState")`.
  - Both DefaultState and SleepState appear as self types.
  - Both are non-inherent (trait) impls.
- Reasoning Trace:
  - Authored Fact (Verbatim): `impl PinState for DefaultState {}` and
    `impl PinState for SleepState {}`.
  - Extracted Fact (Verbatim): Two `ImplBlockFact` entries with
    `trait_name: Some("PinState")` and `self_type_name: "DefaultState"` /
    `self_type_name: "SleepState"`, `is_inherent: false`.
  - Correspondence: `exact`
  - Gap / Loss Description: None.
  - Why This Status Follows: The identity enrichment extraction preserves trait
    implementation facts for both markers. The assertions inspect the exact
    surface carrying type and trait facts and would fail if either impl were
    missing or misclassified.
- Counterexample: N/A
- Conclusion: Pass. PinState trait implementation evidence is preserved for both markers.

### Check 2b: Usage-site distinction

- Check: `apply::<DefaultState>()` and `apply::<SleepState>()` call sites are
  extracted with distinct state_type in the PinCtrl surface.
- Status: `pass`
- Premise: Downstream pinctrl state application can still distinguish the two
  markers.
- Assertion Predicate(s):
  - `result.pinctrl.call_sites.iter().find(|cs| cs.state_type == "DefaultState" && cs.containing_fn == "apply_default_pins_if_present").is_some()`
  - `result.pinctrl.call_sites.iter().find(|cs| cs.state_type == "SleepState" && cs.containing_fn == "apply_sleep_pins_if_present").is_some()`
  - `result.pinctrl.call_sites.len() == 2` (closed-world: exactly two call sites)
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2b_usage_site_distinction` in
    `crates/regseq-driver/tests/pin_state_markers_extraction.rs`, lines 464-497.
  - `apply::<DefaultState>()` found in `apply_default_pins_if_present`.
  - `apply::<SleepState>()` found in `apply_sleep_pins_if_present`.
  - Exactly 2 call sites total (closed-world).
- Reasoning Trace:
  - Authored Fact (Verbatim): `apply_default_pins_if_present` calls
    `pinctrl.apply::<DefaultState>()` and `apply_sleep_pins_if_present` calls
    `pinctrl.apply::<SleepState>()`.
  - Extracted Fact (Verbatim): `PinCtrlCallSite { state_type: "DefaultState", containing_fn: "apply_default_pins_if_present" }`
    and `PinCtrlCallSite { state_type: "SleepState", containing_fn: "apply_sleep_pins_if_present" }`.
  - Correspondence: `exact`
  - Gap / Loss Description: None.
  - Why This Status Follows: The pinctrl surface extraction preserves the call-site
    type argument identity and containing function context. The assertions would
    fail if the state types were swapped, missing, or collapsed.
- Counterexample: N/A
- Conclusion: Pass. Usage-site distinction is preserved.

### Check 2c: Call-site state_type distinctness

- Check: The state_type values at the call sites are distinct, proving downstream
  distinguishability.
- Status: `pass`
- Premise: Marker participation in pinctrl state selection remains distinguishable.
- Assertion Predicate(s):
  - `state_types.contains(&"DefaultState")`
  - `state_types.contains(&"SleepState")`
  - `unique_count == state_types.len()` (all state_types are distinct after dedup)
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2c_call_site_state_types_are_distinct` in
    `crates/regseq-driver/tests/pin_state_markers_extraction.rs`, lines 502-534.
  - Both DefaultState and SleepState appear in state_types.
  - After sort+dedup, count equals original count, proving no duplication.
- Reasoning Trace:
  - Authored Fact (Verbatim): Two distinct markers: DefaultState and SleepState.
  - Extracted Fact (Verbatim): state_types = ["DefaultState", "SleepState"] (after dedup,
    same length).
  - Correspondence: `exact`
  - Gap / Loss Description: None.
  - Why This Status Follows: Direct distinctness verification over the extracted
    call-site state types. The assertion would fail if the two markers produced
    the same state_type string.
- Counterexample: N/A
- Conclusion: Pass. Call-site state_type values are distinct.

## Overall Conclusion

All authored pin-state marker facts for PL011 are preserved at the
compiler-side extraction boundary. Both Check Group 1 (marker-type identity)
and Check Group 2 (PinState implementation evidence and usage-site distinction)
pass with exact correspondence, backed by exercised assertions over the
`IdentityEnrichmentSurface` and `PinCtrlSurface` extraction surfaces.
