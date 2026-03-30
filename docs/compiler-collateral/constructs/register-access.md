# Register Access

## Purpose

`Register Access` defines the typed MMIO surface, declarative register metadata,
 polling intrinsic, and generated region-mapping behavior used by the compiler
 to turn register-centric driver code into backend-specific MMIO operations.

This is the canonical mixed-form resource construct for compiler work.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `register_access.register_block` | property | helper macro / register-block declaration |
| `register_access.access_width` | property | field attribute |
| `register_access.endian` | property | item attribute |
| `register_access.side_effects` | property | field attribute |
| `register_access.hw_updates` | property | field attribute |
| `register_access.mmio_region` | resource | typed field `Mmio<T>` |
| `register_access.read` | operation | wrapper method |
| `register_access.write` | operation | wrapper method |
| `register_access.poll` | operation | `regseq::poll::until(...)` intrinsic |

### Layer 2

| Input | Role |
|---|---|
| physical base address / region size | consumed during MMIO region mapping |

These Layer 2 facts are not Register Access primitives themselves. They are
 joined with the Layer 1 `Mmio<T>` declaration during lowering.

### Compiler-managed

| Primitive | Role |
|---|---|
| `register_access.map_region` | emit map in init-side setup and unmap in deinit-side teardown |

## Accepted Surface Baseline

### Declarative baseline

- register layout and field semantics are declared in Layer 1
- `hw_updates` is the canonical property name

Accepted naming normalization:

- older mapping material used `shadow_registers`
- the canonical compiler-facing primitive name is `hw_updates`

### Wrapper baseline

Stable wrapper-family conventions:

- typed MMIO field declaration is the resource declaration surface
- wrapper methods are the normal surface for resource-local register ops
- `&self` is the default receiver convention for wrapper families
- `modify` and `set` are convenience methods over the core `read` / `write`
  primitive family; they are not separate primitives

Representative baseline:

```rust
impl<T, R> ReadWrite<T, R> {
    fn read(&self, field: Field<T, R>) -> T;
    fn write(&self, values: FieldValue<T, R>);
    fn modify(&self, values: FieldValue<T, R>);
    fn set(&self, values: FieldValue<T, R>);
}
```

### Intrinsic baseline

Stable polling surface:

```rust
regseq::poll::until(condition, delay, timeout) -> Result<(), RegseqError>
```

This remains the accepted free-function intrinsic exception within the wrapper
 baseline.

### Excluded baseline

- `register_access.io_fence` is not part of the stable base surface
- MMIO ordering is carried through compiler constraints and lowering rules
  rather than a dedicated stable Layer 1 primitive

## Extraction Baseline

The frontend must recognize:

- register block declarations and register metadata attributes
- `Mmio<T>` typed fields and their binding identifiers
- wrapper-method calls on known register wrapper types
- `regseq::poll::until(...)` intrinsic calls
- `regseq::ordered!` blocks as an explicit user-facing sequencing surface

Primary specification signals:

- helper macro / register-block declarations for register structure
- typed field declaration for the MMIO resource
- wrapper types and methods for `read` / `write`
- intrinsic call site for `poll`

Required extraction validations:

- access legality from wrapper type (`ReadOnly`, `WriteOnly`, `ReadWrite`)
- side-effect metadata completeness where staging/commit semantics are used
- `hw_updates` and side-effect metadata carried through to lowering

## Lowering Baseline

Lowering ownership:

- declarative metadata: compiler-consumed
- wrapper methods: backend lowers to backend-specific MMIO reads/writes
- `poll`: compiler/backend intrinsic lowering
- `map_region`: compiler-generated from `Mmio<T>` + Layer 2 binding data

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `map_region` | yes | yes | no | no |

Accepted notes:

- `map_region.generated_lifecycle_participation = both`
- shadow-based optimizations are lowering details derived from metadata, not
  separate authored surface
- `regseq::ordered!` remains a stable user-facing ordering macro around
  register operations

## Cross-Construct Dependencies

Depends on:

- `Bus Resource Management` and binding data for physical MMIO location

Feeds:

- `Serial/UART Management`
- `Device interrupt setup and handling`
- `DMA Management`
- `Device power state management` resume shadow flush
- `Hardware reset sequencing and post-reset polling`

Related global constraints:

- MMIO barrier emission is a shared implementation constraint
- poll-loop lowering must honor yield-hint guidance in non-busy contexts

## Constraints and Caveats

- Keep `read` / `write` as wrapper methods, not trait methods.
- Keep `poll` as the free-function intrinsic.
- Preserve the accepted exclusion of `io_fence` from the stable base surface.
- Honor MMIO barrier emission requirements from the shared constraints doc.
- Treat `hw_updates` as the input fact; any shadow register optimization is a
  lowering consequence, not the authored primitive itself.

## Representative Examples

```rust
#[rseq::register_block]
struct Pl011Registers {
    // typed register fields
}

struct Pl011 {
    regs: Mmio<Pl011Registers>,
}

fn wait_for_idle(&self) -> Result<(), RegseqError> {
    regseq::poll::until(
        || self.regs.fr.read(FR_REG::BUSY) == 0,
        Duration::from_us(10),
        Duration::from_ms(100),
    )
}
```

## Implementation Checklist

- Recognize register-block declarations and per-register metadata.
- Recognize `Mmio<T>` typed fields and join them with Layer 2 region facts.
- Lower wrapper-method MMIO access by wrapper type and register metadata.
- Lower `regseq::poll::until(...)` as the stable polling intrinsic.
- Recognize `regseq::ordered!` blocks and preserve their sequencing/barrier
  semantics during lowering.
- Emit `map_region` setup/teardown around device lifetime.
- Carry `hw_updates` and side-effect metadata into lowering decisions.
- Enforce MMIO ordering using shared compiler constraints rather than an
  explicit `io_fence` primitive.
