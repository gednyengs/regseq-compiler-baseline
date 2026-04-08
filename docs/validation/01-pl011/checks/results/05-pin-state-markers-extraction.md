# Validation Certificate: PL011 Pin-State Markers Extraction Check

Source premise: `docs/validation/01-pl011/01-authored-source.md`
Target boundary evidence: `crates/regseq-driver/tests/source_to_extraction_05_pin_state_markers.rs`

## Specimen Fidelity

A reduced inline specimen is used. The specimen nests all authored facts inside
a `pub mod driver { ... }` block to reproduce the authored module structure
(`pl011_full_spec::driver::*`). With the crate name `test_input`, canonical
paths become `test_input::driver::DefaultState` and
`test_input::driver::SleepState`, matching the authored
`<crate>::driver::<Type>` structure exactly.

Specimen fidelity is established by the `precondition_specimen_fidelity` test,
which reads the real fixture source at
`tests/fixtures/pl011-full-spec/src/driver.rs` and asserts that every authored
fact in scope appears verbatim in both the fixture and the specimen, including
the `driver` module nesting structure.

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
- `SPECIMEN.contains("pub mod driver")` -- module nesting matches fixture

Fidelity Evidence (Exact Citations):

- Test `precondition_specimen_fidelity` in
  `crates/regseq-driver/tests/source_to_extraction_05_pin_state_markers.rs`.
- All 17 assertions pass (8 for fixture, 8 for specimen, 1 for module nesting).

## Check-to-Surface Mapping

| Check | Extraction Surface | API |
|---|---|---|
| 1a (distinct canonical paths + zero-sized) | `IdentityEnrichmentSurface.structs` | `extract_identity_enrichment(tcx)` |
| 1a-supp (reconstructable path via module) | `IdentityEnrichmentSurface.structs` (module_path) + `IdentityEnrichmentSurface.modules` | `extract_identity_enrichment(tcx)` |
| 1b (no collapse) | `IdentityEnrichmentSurface.structs` | `extract_identity_enrichment(tcx)` |
| 2a (trait impl evidence) | `IdentityEnrichmentSurface.impl_blocks` | `extract_identity_enrichment(tcx)` |
| 2b (usage-site distinction) | `PinCtrlSurface.call_sites` | `extract_pinctrl_surface(tcx)` |
| 2c (state_type distinctness) | `PinCtrlSurface.call_sites` | `extract_pinctrl_surface(tcx)` |

## Check Group 1: Marker-type identity remains distinct

### Check 1a: Both marker types are extracted as distinct structs

- Check: DefaultState and SleepState are extracted as distinct structs with
  full canonical paths and zero-sized property in the identity enrichment
  surface.
- Status: `pass`
- Premise: Both marker types remain identifiable and distinct.
- Assertion Predicate(s):
  - `result.identity.structs.iter().find(|s| s.name == "DefaultState" && s.module_path.0 == "driver").is_some()`
  - `result.identity.structs.iter().find(|s| s.name == "SleepState" && s.module_path.0 == "driver").is_some()`
  - `ds.canonical_path != ss.canonical_path` (non-collapse)
  - `ds.canonical_path.0 == "driver::DefaultState"` (exact full canonical path)
  - `ss.canonical_path.0 == "driver::SleepState"` (exact full canonical path)
  - `ds.name == "DefaultState"` (exact struct name)
  - `ss.name == "SleepState"` (exact struct name)
  - `crate_root.name == "test_input"` (crate root existence)
  - `format!("{}::{}", crate_root.name, ds.canonical_path.0) == "test_input::driver::DefaultState"` (full path reconstruction)
  - `format!("{}::{}", crate_root.name, ss.canonical_path.0) == "test_input::driver::SleepState"` (full path reconstruction)
  - `ds.fields.is_empty()` (DefaultState is zero-sized)
  - `ss.fields.is_empty()` (SleepState is zero-sized)
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check1a_marker_types_have_distinct_canonical_paths` in
    `crates/regseq-driver/tests/source_to_extraction_05_pin_state_markers.rs`.
  - Both structs found in `IdentityEnrichmentSurface.structs` with:
    - Exact canonical path equality (`driver::DefaultState`, `driver::SleepState`)
    - Full path reconstructability via crate root module (`test_input::driver::DefaultState`, `test_input::driver::SleepState`)
    - Zero field count proving zero-sized marker type property
- Reasoning Trace:
  - Authored Fact (Verbatim): `pub struct DefaultState;` and `pub struct SleepState;`
    as zero-sized marker types with canonical paths
    `pl011_full_spec::driver::DefaultState` and `pl011_full_spec::driver::SleepState`.
  - Extracted Fact (Verbatim): `StructFact { name: "DefaultState", canonical_path: CanonicalPath("driver::DefaultState"), module_path: CanonicalPath("driver"), fields: [] }`
    and `StructFact { name: "SleepState", canonical_path: CanonicalPath("driver::SleepState"), module_path: CanonicalPath("driver"), fields: [] }`.
  - Correspondence: `exact` -- the `<crate>::driver::<Type>` path structure is
    fully proven via exact canonical path equality plus crate root reconstruction.
    The only difference is the crate name (`test_input` vs `pl011_full_spec`),
    which is expected since we use a reduced specimen. The zero-sized property is
    proven by asserting empty field sets.
  - Gap / Loss Description: None. The canonical path omits the local crate
    prefix (a rustc `def_path_str` convention), but the crate root module
    provides the prefix for exact reconstruction.
  - Why This Status Follows: The assertions directly inspect the extraction
    surface with exact equality (not suffix-only). The full path is
    reconstructed and proven equal to the expected `test_input::driver::DefaultState`
    structure. The zero-sized property is verified by asserting empty fields.
    All assertions would fail if the authored facts were not preserved.
- Counterexample: N/A
- Conclusion: Pass. Marker-type identity, full canonical paths, and zero-sized
  property are all preserved.

### Check 1a (supplementary): Full canonical path reconstructability via module

- Check: The full canonical path is reconstructable from `module_path + "::" + name`
  and equals the extraction's `canonical_path` exactly.
- Status: `pass`
- Premise: Full authored canonical paths for DefaultState and SleepState are
  exactly reconstructable from the extraction surface.
- Assertion Predicate(s):
  - `ds.module_path.0 == "driver"` (exact module path)
  - `ss.module_path.0 == "driver"` (exact module path)
  - `format!("{}::{}", ds.module_path.0, ds.name) == ds.canonical_path.0` (reconstruction equals canonical)
  - `format!("{}::{}", ss.module_path.0, ss.name) == ss.canonical_path.0` (reconstruction equals canonical)
  - `ds_reconstructed != ss_reconstructed` (distinct)
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check1a_canonical_path_reconstructable_via_module` in
    `crates/regseq-driver/tests/source_to_extraction_05_pin_state_markers.rs`.
  - Reconstructed path `driver::DefaultState` equals `canonical_path.0` exactly.
  - Reconstructed path `driver::SleepState` equals `canonical_path.0` exactly.
- Reasoning Trace:
  - Authored Fact (Verbatim): Canonical paths `pl011_full_spec::driver::DefaultState`
    and `pl011_full_spec::driver::SleepState`.
  - Extracted Fact (Verbatim): `module_path: "driver"`, `name: "DefaultState"` /
    `"SleepState"`. Reconstructed paths equal canonical paths exactly.
  - Correspondence: `exact`
  - Gap / Loss Description: None. The surface provides all components needed
    for exact reconstruction.
  - Why This Status Follows: Exact equality assertions between reconstructed
    and canonical paths prove the surface is self-consistent and the full
    module nesting is preserved.
- Counterexample: N/A
- Conclusion: Pass. Full canonical paths are exactly reconstructable.

### Check 1b: No marker collapse (closed-world)

- Check: The complete set of structs in the `driver` module named DefaultState
  or SleepState contains exactly one of each.
- Status: `pass`
- Premise: DefaultState and SleepState do not collapse.
- Assertion Predicate(s):
  - `driver_structs.iter().filter(|s| s.name == "DefaultState").count() == 1`
  - `driver_structs.iter().filter(|s| s.name == "SleepState").count() == 1`
  - `marker_names.len() == 2` (closed-world total)
  - where `driver_structs` is filtered by `module_path.0 == "driver"`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check1b_no_marker_collapse_closed_world` in
    `crates/regseq-driver/tests/source_to_extraction_05_pin_state_markers.rs`.
  - Closed-world enumeration within the `driver` module confirms exactly one
    DefaultState and exactly one SleepState. Neither is duplicated, dropped,
    or merged within the module scope.
- Reasoning Trace:
  - Authored Fact (Verbatim): Two distinct zero-sized marker types
    `DefaultState` and `SleepState`.
  - Extracted Fact (Verbatim): Exactly two structs with these names exist in
    the `driver` module scope of the extraction.
  - Correspondence: `exact`
  - Gap / Loss Description: None.
  - Why This Status Follows: The closed-world count assertions would fail if
    either marker were missing, duplicated, or if one collapsed into the other.
- Counterexample: N/A
- Conclusion: Pass. No marker collapse.

## Check Group 2: PinState implementation evidence survives

### Check 2a: PinState trait impl evidence

- Check: Trait impl blocks for `PinState` exist for both marker types in the
  identity enrichment surface.
- Status: `pass`
- Premise: PinState implementation evidence survives for each marker.
- Assertion Predicate(s):
  - `pinstate_impls.len() == 2` (exactly two distinct PinState trait impls in driver module, after dedup)
  - `impl_self_types.contains(&"DefaultState")` (DefaultState covered)
  - `impl_self_types.contains(&"SleepState")` (SleepState covered)
  - `!ib.is_inherent` (both are trait impls, not inherent)
  - `ib.trait_path.is_some()` (both carry trait path)
  - Impls filtered by `self_type_path.0.starts_with("driver::")` and deduplicated
    by `(self_type_name, canonical_path)` to select module-scoped facts.
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2a_pinstate_trait_impl_evidence` in
    `crates/regseq-driver/tests/source_to_extraction_05_pin_state_markers.rs`.
  - Two distinct `ImplBlockFact` entries with `trait_name == Some("PinState")`
    found, one for each marker type in the `driver` module.
- Reasoning Trace:
  - Authored Fact (Verbatim): `impl PinState for DefaultState {}` and
    `impl PinState for SleepState {}`.
  - Extracted Fact (Verbatim): Two `ImplBlockFact` entries with
    `trait_name: Some("PinState")`, `self_type_name: "DefaultState"` and
    `self_type_name: "SleepState"` respectively, with
    `self_type_path` starting with `"driver::"`.
  - Correspondence: `exact`
  - Gap / Loss Description: None.
  - Why This Status Follows: The trait impl facts are directly extracted and
    carry both the trait name and self-type name, exactly matching the authored
    declarations.
- Counterexample: N/A
- Conclusion: Pass. PinState trait implementation evidence preserved.

### Check 2b: Usage-site distinction

- Check: `apply::<DefaultState>()` and `apply::<SleepState>()` call sites are
  extracted with distinct state_type values, each in the correct containing
  function.
- Status: `pass`
- Premise: Downstream pinctrl state application can still distinguish the
  two markers.
- Assertion Predicate(s):
  - `result.pinctrl.call_sites.iter().find(|cs| cs.state_type == "DefaultState" && cs.containing_fn == "apply_default_pins_if_present").is_some()`
  - `result.pinctrl.call_sites.iter().find(|cs| cs.state_type == "SleepState" && cs.containing_fn == "apply_sleep_pins_if_present").is_some()`
  - `result.pinctrl.call_sites.len() == 2` (closed-world, no duplication/loss)
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2b_usage_site_distinction` in
    `crates/regseq-driver/tests/source_to_extraction_05_pin_state_markers.rs`.
  - Two `PinCtrlCallSite` entries found with the exact authored state_type and
    containing_fn pairs.
- Reasoning Trace:
  - Authored Fact (Verbatim):
    - `apply_default_pins_if_present` calls `pinctrl.apply::<DefaultState>()?`
    - `apply_sleep_pins_if_present` calls `pinctrl.apply::<SleepState>()?`
  - Extracted Fact (Verbatim):
    - `PinCtrlCallSite { state_type: "DefaultState", containing_fn: "apply_default_pins_if_present" }`
    - `PinCtrlCallSite { state_type: "SleepState", containing_fn: "apply_sleep_pins_if_present" }`
  - Correspondence: `exact`
  - Gap / Loss Description: None.
  - Why This Status Follows: The call-site extraction preserves both the
    turbofish type argument and the containing function name, exactly matching
    the authored usage sites.
- Counterexample: N/A
- Conclusion: Pass. Usage-site distinction preserved.

### Check 2c: State-type distinctness at call sites

- Check: The state_type values at the call sites are distinct.
- Status: `pass`
- Premise: No marker state-type confusion at the usage-site level.
- Assertion Predicate(s):
  - `state_types.contains(&"DefaultState")`
  - `state_types.contains(&"SleepState")`
  - `sorted.len() == state_types.len()` (all distinct after dedup)
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2c_call_site_state_types_are_distinct` in
    `crates/regseq-driver/tests/source_to_extraction_05_pin_state_markers.rs`.
  - After dedup, the state_type set retains the same cardinality, proving
    no two call sites share a state_type.
- Reasoning Trace:
  - Authored Fact (Verbatim): `DefaultState` and `SleepState` are used as
    distinct type arguments at `apply` call sites.
  - Extracted Fact (Verbatim): `state_type` values `"DefaultState"` and
    `"SleepState"` are present and distinct.
  - Correspondence: `exact`
  - Gap / Loss Description: None.
  - Why This Status Follows: The dedup cardinality assertion would fail if
    any two call sites shared the same state_type string.
- Counterexample: N/A
- Conclusion: Pass. State-type values are distinct at the usage-site level.

## Summary

All checks pass. The extraction boundary preserves:
- Distinct type identity for both `DefaultState` and `SleepState` with exact
  full canonical paths (`driver::DefaultState`, `driver::SleepState`) and
  full reconstructability (`test_input::driver::DefaultState`, etc.) (Check 1a)
- Full canonical path reconstruction consistency (Check 1a-supp)
- Zero-sized marker type property (empty field sets) (Check 1a)
- No marker collapse under closed-world enumeration (Check 1b)
- PinState trait implementation evidence for both markers (Check 2a)
- Usage-site distinction with correct state_type/containing_fn pairing (Check 2b)
- Distinct state_type values across all call sites (Check 2c)
