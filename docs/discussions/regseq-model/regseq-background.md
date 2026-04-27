# Regseq Background

Regseq is a DSL and compiler for authoring hardware device drivers in an
OS-agnostic way. Driver authors write specifications using the Regseq DSL
(embedded in Rust), and the compiler generates target OS-specific code (e.g.,
C for Linux, Zephyr, etc.).

A driver specification has two layers:
- **Layer 1 (spec)**: Captures the platform-independent aspects of the driver in the Rust DSL. These are `.rs` Rust files.
- **Layer 2 (platform file)**: Captures platform-specific bindings such as MMIO
  addresses, interrupt line numbers, clock frequencies, etc. Analogous to device
  tree or ACPI tables in a separate platform file. This is a `.toml` file

The DSL provides three mechanisms for capturing device driver semantics:
- **Regseq intrinsics** (the verbs): The operational vocabulary. Calls to
  pre-specified functions (`busy_wait()`, `sleep()`, `log_info()`,
  `poll_until()`) that the compiler materializes into target OS-specific
  mechanisms. The spec author says WHAT should happen; the compiler implements
  the HOW.
- **Regseq properties** (the adjectives): The declarative vocabulary. Custom
  attributes (`#[rseq::mmio(access_width = 32)]`, `#[rseq::interrupt(trigger_type = "...")]`,
  etc.) attached to Regseq abstractions. These emerged from taxonomy research
  that identified the cross-OS commonalities in how resources are described.
- **Regseq traits** (the integration surfaces): The structural contracts.
  Callback families (`Lifecycle`, `Serial`, `PowerManagement`) that define
  the shapes OS frameworks need to integrate a driver. The compiler uses these
  to generate the right registration and hookup code for each target.

The `#[rseq::]` namespace is the syntactic mechanism that carries properties,
but the three concepts are distinct.

## Spec environment

Driver specifications are `#![no_std]` Rust crates. Only a limited subset of `core` is
available. `std` is not used. This mirrors embedded systems Rust
programming and ensures the spec does not depend on OS-specific runtime
facilities (heap allocation, I/O, threading) that are incompatible with
the driver domain.

- **`Rust core`**: Rust's core library. Provides `Option`, `Result`, basic
  traits (`Add`, `BitOr`, `Deref`, etc.), primitive types, and
  various language traits such as `Copy`, `Deref`, `Drop`, etc.
- **`regseq_macros`**: Provides procedural macros that make it easier to define some Regseq constructs.
  These macros are completely expanded into valid Regseq rust code that uses the supported subset of Rust and
  Regseq constructs.
- **`driver crate`**: This have two parts:
  - The **regseq library**: this contains the primitives provided by the Regseq DSL. Driver authors use these primitives
    to author device driver specifications. To avoid issues related to cross-crate resolutions, this should be a sub-module in the 
    driver crate.
  - The **driver specification**: this is the authored device driver specification that uses the **regseq library** and various Regseq-specific
    attributes (`#[rseq::]`) to describe intent driver intent.
  