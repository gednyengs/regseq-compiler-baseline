//! Forward-declared value types for stages 3, 4, 11, and 12.
//!
//! Step 6 lands the top-level `RegseqModel` struct, which carries
//! `IndexVec<TyId, Ty>`, `IndexVec<ConstId, Const>`,
//! `IndexVec<ItemId, Item>`, `IndexVec<FnSigId, FnSig>`, and
//! `IndexVec<BodyId, Body>` -- but the *value types* in those
//! tables (`Ty`, `Const`, `Item`, `FnSig`, `Body`) are defined by
//! later stages:
//!
//! - `Ty` is defined in stage 3 (Type system).
//! - `Const` is defined in stage 4 (Constants and literals).
//! - `Item` is defined in stage 12 (Rust Items).
//! - `FnSig` and `Body` are defined in stage 11 (MIR bodies).
//!
//! Stage 1 ships them as empty braced placeholder structs so the
//! `RegseqModel` field set compiles and can be exercised end-to-end
//! by foundation tests. The empty-struct shape was deliberate: the
//! braced form (`pub struct X {}`) keeps named-field syntax open
//! for stage-N expansion without requiring callers to switch from
//! `X` to `X { .. }`. On the wire, an empty braced struct is `{}`
//! (a JSON object), which extends naturally as fields are added.
//!
//! Stage-1 RegseqModels have *empty* tables for these types -- the
//! placeholder is there to make the struct definition cohere, not
//! to be instantiated. Per methodology ("re-derive from evidence;
//! do not import baggage"), the eventual stage-N expansion is not
//! drawn from `regseq-model-old`; it is built bottom-up from
//! MIR/HIR evidence.

use serde::{Deserialize, Serialize};

/// Forward-declared `Ty`. Defined fully in stage 3 (Type system).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ty {}

/// Forward-declared `Const`. Defined fully in stage 4 (Constants).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Const {}

/// Forward-declared `Item`. Defined fully in stage 12 (Rust Items).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Item {}

/// Forward-declared `FnSig`. Defined fully in stage 11 (MIR bodies).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FnSig {}

/// Forward-declared `Body`. Defined fully in stage 11 (MIR bodies).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Body {}
