# Linux Driver Layout Strategy

## Purpose

This note defines the recommended first Linux code-generation layout for
Regseq-generated drivers.

It exists because Phase D task authoring needs a concrete answer to a question
that sits between lowering semantics and backend implementation:

- what exact Linux-facing artifact shape should the compiler emit first
- what belongs in generated output versus stable handwritten support
- what layout best supports the regenerate/build/fix loop on Ubuntu

This note is about the first Linux bring-up path, not the final polished
kernel-tree integration story for the full paper artifact.

## Design Constraints

The first Linux code-generation strategy should satisfy these constraints:

1. It must preserve a clear boundary between the lowering-manifest and emitted
   Linux artifacts.
2. It must support an agent-friendly regenerate/build/fix loop on Ubuntu 24.04.
3. It must not require handwritten per-driver wrapper glue just to make the
   generated driver compile.
4. It must remain reviewable by humans, including explicit module and driver
   registration scaffolding.
5. It must fit the first PL011 Linux path, which is platform-driver shaped.
6. It must not assume the first backend iteration already deserves in-tree
   Linux integration.

## Candidate Layouts

### Option A: single generated translation unit

Emit one generated `.c` file that contains:

- Linux includes
- module metadata
- driver-private structs
- probe/remove callbacks
- device-class integration
- registration boilerplate
- all lowered helper code

Pros:

- simplest build story
- easiest artifact to inspect
- no handwritten wrapper glue
- good for early backend debugging

Cons:

- can become large quickly
- later extraction into cleaner multi-file structure may be needed

### Option B: generated core plus handwritten wrapper

Emit a generated driver core but rely on a handwritten Linux wrapper for:

- module registration
- includes
- `platform_driver` scaffolding
- build glue

Pros:

- smaller backend scope at first

Cons:

- creates per-driver handwritten coupling
- blurs responsibility between compiler and integration layer
- weakens traceability from lowering output to compiled Linux artifact
- makes agent feedback loops less deterministic

### Option C: in-tree-like multi-file generated layout

Emit several generated files that mimic an in-tree Linux driver layout from the
start.

Pros:

- closer to the eventual Linux artifact shape
- may better match final paper packaging

Cons:

- more emitter complexity immediately
- slower debug loop
- more moving parts before the first compile/build proof exists

## Recommended First Strategy

Use a constrained multi-file out-of-tree layout:

- keep one fixed backend-owned Linux integration/scaffold fileset
- partition the remaining generated implementation files by authored
  source-module provenance
- generate deterministic build metadata needed for an out-of-tree module build
- avoid handwritten per-driver wrapper glue in the first iteration

This preserves a stable, inspectable artifact boundary:

1. joined lowering input
2. lowering manifest
3. generated Linux integration/scaffold fileset
4. generated source-module-partitioned implementation filesets

This is a better first Phase D shape than a single large generated C file
because it keeps debug locality high for both humans and LLM agents while still
keeping the Linux integration boundary explicit and backend-owned.

## Recommended Artifact Boundary

For the first Linux backend iteration, the generated output should own:

- device-specific structs and state layout
- Linux callback bodies derived from Regseq operations
- resource acquisition and cleanup scaffolding
- `platform_driver` registration and match table scaffolding
- device-class integration scaffolding such as serial registration
- module metadata that is directly tied to the generated driver
- lowering-manifest or equivalent debug artifact
- deterministic module build metadata needed for the default debug workflow

The first Linux backend iteration should not rely on handwritten
device-specific glue for:

- probe/remove registration
- match-table registration
- serial device-class hookup
- per-driver Makefile/Kbuild authoring

One explicit split is required:

- the Linux integration/registration shell stays in a fixed backend-owned
  fileset
- lowered device implementation is partitioned by authored source-module
  provenance

This means authored source modules influence the final generated file layout,
but do not control the OS-integration scaffold itself.

## Recommended Generated File Set

The first output layout should look roughly like:

```text
<output-root>/linux/<driver-id>/
  regseq_<driver-id>_module.c
  regseq_<driver-id>_shared.h
  <module-a>.c
  <module-a>.h
  <module-b>.c
  <module-b>.h
  regseq_<driver-id>.manifest.json
  Makefile
  Kbuild
```

Where:

- `regseq_<driver-id>_module.c` is the fixed backend-owned Linux integration
  unit
- `regseq_<driver-id>_shared.h` is optional but recommended when common state,
  shared declarations, or cross-unit Linux-facing helper declarations need a
  deterministic home
- `<module-x>.c/.h` pairs are generated from authored top-level source-module
  provenance when those modules own meaningful lowered content

The backend should not emit empty or trivial module pairs purely because a Rust
module exists. The partitioning rule should be provenance-based but still
filtered by whether that module actually owns emitted content.

## Generated Versus Handwritten Boundary

Generated:

- driver-specific Linux integration/scaffold source
- driver-specific implementation units grouped by source-module provenance
- driver-specific headers for cross-unit access where needed
- driver-specific registration boilerplate
- driver-specific build metadata
- driver-specific debug/manifest artifacts

Handwritten but stable and shared:

- outer evaluation/build scripts used on Ubuntu
- optional generic support headers or runtime helpers only if multiple drivers
  truly need them
- benchmark or deployment harness glue outside the compiler-emitted artifact

Not allowed in the first strategy:

- handwritten PL011-specific wrapper files whose only purpose is to make the
  generated driver build or register correctly

## Integration Mode Recommendation

The first Linux backend should target out-of-tree module integration.

Reasons:

- shorter compile/debug cycle
- simpler artifact boundary
- easier deterministic rebuilds for agents
- avoids full-kernel rebuilds while the backend shape is still moving

Moving toward in-tree replacement remains a later option once the emitted
artifact shape stabilizes.

## Linux-Specific Realization Guidance

For the first PL011 Linux path:

- emit `platform_driver`-style registration directly in the backend-owned
  integration fileset
- emit module metadata directly in the backend-owned integration fileset
- keep Linux includes and macros backend-owned rather than hidden in
  handwritten wrapper files
- keep cleanup paths explicit and reviewable, even if later helper factoring is
  introduced

The generated module partitioning rule should be:

1. one fixed backend-owned scaffold/integration fileset is always emitted
2. remaining implementation files are grouped by authored top-level source
   module provenance
3. shared declarations go either into the owning module header or the shared
   backend-owned header

This rule is deterministic and compiler-friendly because it depends on explicit
authored provenance rather than backend heuristics about "good" file splits.

If a shared Linux support layer becomes necessary, it should be:

- generic rather than PL011-specific
- checked in once
- small enough that the generated-versus-support boundary remains obvious

## Non-Goals

This note does not settle:

- the final in-tree Linux artifact shape
- the final paper packaging strategy
- Zephyr code-generation layout
- E1000-specific Linux layout details
- benchmark harness structure
- full Linux style-polish beyond what is needed for early backend reviewability

## Immediate Recommendation

Phase D Linux task authoring should assume:

1. one fixed generated Linux integration/scaffold fileset per driver
2. zero or more generated implementation filesets derived from authored
   top-level source-module provenance
3. one generated lowering-manifest or equivalent debug artifact beside them
4. generated Makefile/Kbuild for out-of-tree module bring-up
5. no handwritten per-driver wrapper glue
6. platform-driver registration emitted directly by the Linux backend

This is the best first shape for PL011 Linux lowering, backend debugging, and
LLM-agent iteration on Ubuntu.
