# Zephyr Evaluation And Development Strategy

## Purpose

This note captures the Zephyr-side development, validation, and evaluation
strategy for Regseq-generated drivers.

It lives alongside the Linux/QEMU notes because both target environments shape:

- backend development workflow
- agent debugging workflow
- what evaluation claims the paper can make

## Key Difference From Linux

Zephyr does not need the same kernel-module or kernel-tree replacement story as
Linux.

The core Zephyr workflow is more naturally:

1. generate Zephyr-target driver collateral
2. place it into the chosen Zephyr application/module layout
3. build with the Zephyr toolchain (`west`, `cmake`, `ninja`)
4. inspect compiler/build errors
5. patch the lowering/backend logic
6. rebuild

That makes the compile/fix loop structurally simpler than Linux.

## Host Environment

Use Ubuntu 24.04 as the source-of-truth environment here too.

That gives us:

- one consistent host setup across Linux and Zephyr backend work
- easier toolchain management
- easier scripting for agent-driven generate/build/fix loops

## Validation Layers

### 1. Compile-time validation

This is the first and most important Zephyr loop:

- generated output integrates into the Zephyr build graph
- the selected board/application builds successfully
- Kconfig/devicetree/driver integration errors are caught early

### 2. QEMU-based smoke validation

Where Zephyr supports QEMU for the chosen board:

- boot the generated driver in QEMU
- confirm init succeeds
- confirm minimal device behavior works

This should be the first runtime-oriented validation path.

### 3. Board-native validation

Use real hardware only if later needed for:

- stronger runtime confidence
- paper claims that cannot be supported by QEMU-based Zephyr evaluation alone

It should not be the first dependency for backend bring-up.

## Agent-Friendly Development Loop

The Zephyr workflow should explicitly support an LLM agent loop:

1. generate Zephyr-target collateral
2. place it into the selected Zephyr layout
3. run the canonical Zephyr build
4. collect errors and warnings deterministically
5. patch lowering/backend logic
6. regenerate and rebuild

This requires a deterministic build harness with:

- one canonical generated-artifact location
- one canonical Zephyr workspace/app layout
- one canonical target board per early backend task
- one canonical build command
- deterministic log/artifact locations

## Early Default Strategy

For early Zephyr backend work, prefer:

- one fixed QEMU-supported Zephyr board when possible
- one small application harness
- one deterministic `west build` path

That is the shortest path to:

- generated-code compile validation
- QEMU smoke validation
- agent-driven error/fix iteration

## Evaluation Claim Boundary

The same claim discipline applies here as on Linux:

- prefer controlled relative comparisons under the same environment
- avoid overstating absolute physical-hardware performance claims

If Zephyr evaluation later becomes part of the paper, the methodology should
make clear:

- what is QEMU-based
- what is board-native
- what is relative comparison versus absolute device performance

## Follow-on Questions

The next Zephyr-specific decisions we will need are:

- which Zephyr board is the source-of-truth target for early backend work
- whether that board is QEMU-runnable
- what the minimal generated-driver integration shape is
- what the canonical `west build` command is
- what runtime smoke signal we require before calling the Zephyr path healthy
