# PinCtrl Discussion

## Purpose

This note captures the current Regseq design discussion around the `PinCtrl`
typed resource surface, its compile-time state-marker contract, and the missing
details in the Layer 1 to Layer 2 mapping story.

The immediate goal is to make the `PinCtrl::apply::<S>()` surface more precise
before later Layer 2 binding and lowering work depends on it.

## Current Accepted Baseline

The accepted construct baseline in
`docs/compiler-collateral/constructs/pin-controller-and-gpio-configuration.md`
currently gives us:

- typed resource field:
  - `PinCtrl`
  - `Option<PinCtrl>`
- wrapper operation:
  - `PinCtrl::apply<S: PinState>(&self) -> Result<(), RegseqError>`
- Layer 2 property:
  - `pin_states`
- default Layer 1 to Layer 2 join:
  - field-name join
- optional override:
  - `#[rseq::pinctrl(id = "...")]`

It also says:

- `apply::<S>()` must use a compile-time-valid state marker from the joined
  Layer 2 state inventory
- backend lowering owns the actual pinctrl state-selection API call
- compiler-generated `acquire_pinctrl` belongs only to the init preamble
- PM owns compiler-generated suspend/resume pin-state transitions

## Current Problem

The current accepted construct says that `apply::<S>()` must use a valid state
marker from the joined Layer 2 inventory, but it does **not** yet define the
exact mapping rule from the Rust type marker `S` to the Layer 2 state entry.

So today we have:

- a declared generic marker contract
- a declared Layer 2 inventory
- but no formal bridge between them

This leaves an important gap in the specification and eventual lowering logic.

## What `S` Is Intended To Be

`S` is not intended to be a runtime value.

The current shape strongly suggests that state markers are:

- zero-sized compile-time marker types
- most naturally unit structs
- implementing `PinState`

For example:

```rust
pub struct Default;
pub struct Sleep;

impl PinState for Default {}
impl PinState for Sleep {}
```

That is a natural fit for:

```rust
self.pinctrl.apply::<Default>()?;
self.pinctrl.apply::<Sleep>()?;
```

This means:

- `Default` and `Sleep` are types, not enum variants or runtime values
- the type argument communicates state selection intent
- no runtime state object is passed

## Leading Design Direction

The strongest current direction is to make the state-to-Layer-2 mapping
explicit in the `PinState` trait itself, rather than relying on implicit type
name normalization.

A candidate trait shape is:

```rust
pub trait PinState {
    const STATE_NAME: &'static str;
}
```

with markers such as:

```rust
pub struct Default;
impl PinState for Default {
    const STATE_NAME: &'static str = "default";
}

pub struct Sleep;
impl PinState for Sleep {
    const STATE_NAME: &'static str = "sleep";
}
```

## Why This Is Better

This direction is attractive because it:

- makes the Layer 1 to Layer 2 bridge explicit
- avoids hidden type-name normalization rules
- avoids questions like:
  - `Default` vs `DefaultState`
  - `SleepPins` vs `Sleep`
  - case normalization
- gives the compiler a direct string key to validate against the joined
  Layer 2 `pin_states`

Instead of:

- infer string key from Rust type name

the compiler can do:

1. resolve the `PinCtrl` field
2. join it to the Layer 2 `pin_states` inventory
3. resolve the marker type `S`
4. read `S::STATE_NAME`
5. validate that `STATE_NAME` exists in the joined Layer 2 inventory

That is a much cleaner contract for both validation and lowering.

## Alternative Direction

The main alternative is implicit type-name mapping, for example:

- `Default` -> `default`
- `Sleep` -> `sleep`

possibly with additional normalization rules such as:

- strip `State` suffix
- lowercase
- underscore conversion

This is less attractive because:

- it hides the mapping rule
- it creates naming edge cases
- it makes the compiler's behavior less explicit
- it becomes harder to explain and justify in the specification

## Lowering Story With Explicit `STATE_NAME`

If `PinState` carries `STATE_NAME`, then the lowering path becomes:

1. authored code calls:

```rust
self.pinctrl.apply::<Default>()?;
```

2. frontend extraction captures:
   - pinctrl field
   - concrete marker type `Default`
3. Layer 1 / Layer 2 join resolves the relevant `pin_states` inventory
4. compiler reads:

```rust
Default::STATE_NAME == "default"
```

5. compiler validates that `"default"` is present in the joined Layer 2
   inventory
6. backend lowering emits the target-native pinctrl state-selection call for
   that resolved state entry

This makes the path from authored type marker to target-native pin state much
more explicit.

## What Is Still Open

### 1. Should `PinState` require only `STATE_NAME`?

Or should it later carry more metadata, such as:

- whether the state is intended for PM sleep
- whether the state is default-only

For now, `STATE_NAME` alone looks like the right minimum.

### 2. Should the facade task be widened now?

Current Phase-B task specs deliberately keep `PinState` minimal and do not yet
tie it to Layer 2 validation.

So we need to decide whether:

- this associated-constant design should be adopted immediately in the facade
  task
- or recorded now and introduced in a later refinement batch

### 3. Who defines the marker types?

Possible models:

- user-defined marker types implementing `PinState`
- compiler-generated marker types derived from Layer 2
- a mix of both

The current surface leans toward:

- user-defined or crate-defined marker types in authored code

but the exact ownership model is still open.

## Current Recommendation

Treat the lack of an explicit `PinState` to Layer 2 mapping contract as a real
spec gap.

The current leading direction is:

- `PinState` should eventually require:

```rust
const STATE_NAME: &'static str;
```

- that constant should be the formal bridge used to validate
  `apply::<S>()` against the joined Layer 2 `pin_states`

## Status

Open discussion. The explicit `PinState::STATE_NAME` direction is the current
leading candidate for closing the Layer 1 / Layer 2 mapping gap.
