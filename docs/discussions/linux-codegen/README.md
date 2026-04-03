# Linux Codegen Discussion

This directory is the discussion area for Linux code generation strategy in
the Regseq compiler.

It exists to settle questions that sit between lowering semantics and concrete
generated Linux artifact shape, including:

- what the first generated Linux driver layout should be
- what is generated versus handwritten wrapper or integration glue
- whether the first iteration targets out-of-tree module layout, in-tree style
  layout, or a staged hybrid
- how generated files should be partitioned across C sources, headers, build
  metadata, and debug artifacts
- where module registration, platform-driver scaffolding, and device-class
  integration should live
- what canonical output directory/layout should support the regenerate/build/fix
  loop on Ubuntu

Current direction:

- Linux-first code generation should optimize for an agent-friendly
  regenerate/build/fix loop before aiming for final polished kernel-tree
  integration
- the first backend iteration should likely target an out-of-tree module style
  integration layout unless a stronger implementation reason forces a different
  choice
- the generated artifact boundary should remain reviewable and traceable back
  to the lowering-manifest or equivalent debug artifact

Follow-on notes in this directory should define:

- the canonical generated Linux artifact layout
- the division between generated code and handwritten support glue
- the first module/build packaging strategy
- output-location and artifact-capture conventions for backend iteration
- any Linux-specific readability or reviewability requirements for generated C

Current repo-local notes:

- [linux-driver-layout-strategy.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/linux-codegen/linux-driver-layout-strategy.md)
