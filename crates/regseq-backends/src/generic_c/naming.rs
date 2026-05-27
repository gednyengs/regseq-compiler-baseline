//! D1 type-name derivation (`02-architecture-decisions.md` D1).
//!
//! A pure naming layer used by the `Ty`->C spelling ([`super::types`]) and
//! the section 14 aggregate emitter ([`super::layout_emit`]). Aggregate
//! **type** names are *derived* -- `sanitize(DefPath.rendered)` plus a
//! `generic_suffix(ArgsKey)` -- so they are a pure function of the model
//! and byte-stable; instance symbols (fn / static / const `emission_name`)
//! are out of scope for this task (bank 4/5, task d).
//!
//! Collisions are impossible in a well-formed model because `(DefPath,
//! ArgsKey)` is the nominal identity key; the `_clash_<n>` disambiguation
//! pass in [`super::NameTable`] is the safety net.

use regseq_model::{args_key_for_generic_args, GenericArgs, RegseqModel, Ty, TyId};

/// Sanitize a rendered path into a C identifier body per D1: collapse
/// `::` to a single `_`, then map every remaining non-`[A-Za-z0-9_]`
/// byte to `_`. Total and deterministic.
pub(crate) fn sanitize(s: &str) -> String {
    s.replace("::", "_")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The D1 `generic_suffix(ArgsKey)`: empty for a non-generic item
/// (`args` empty), otherwise `__<hex>` of the canonical `ArgsKey` (the
/// same key the interner uses, so it is post-mono-unique and
/// build-stable).
fn generic_suffix(model: &RegseqModel, args: &GenericArgs) -> String {
    if args.is_empty() {
        String::new()
    } else {
        let key = args_key_for_generic_args(model, args);
        format!("__{:x}", key.0)
    }
}

/// The derived base name (pre-`_clash_<n>`) for an aggregate `ty`.
///
/// `Struct` / `Enum` / `Union` / `Closure` derive from the owning item's
/// `DefPath.rendered` + the generic suffix; tuples derive a stable
/// `tuple_<hash>` from their field `TyId`s (fields are then `_0`, `_1`,
/// ...). Panics if `ty` is not an aggregate -- callers only reach this
/// for discovered aggregate nodes.
pub(crate) fn aggregate_base_name(model: &RegseqModel, ty: TyId) -> String {
    match &model.types[ty] {
        Ty::Struct { item, args, .. }
        | Ty::Enum { item, args, .. }
        | Ty::Union { item, args, .. } => {
            let rendered = model.strings.resolve(model.items[*item].def_path.rendered);
            format!("{}{}", sanitize(rendered), generic_suffix(model, args))
        }
        Ty::Closure(item, args) => {
            // The parallel capture-struct name (D1 #3): same item + args.
            let rendered = model.strings.resolve(model.items[*item].def_path.rendered);
            format!("{}{}", sanitize(rendered), generic_suffix(model, args))
        }
        Ty::Tuple { fields, .. } => tuple_base_name(fields),
        other => panic!("aggregate_base_name on non-aggregate Ty: {other:?}"),
    }
}

/// A stable `tuple_<hash>` name from the field `TyId`s (FNV-1a over the
/// little-endian field indices). Deterministic within a model, hence
/// byte-stable across two emits of the same model.
fn tuple_base_name(fields: &[TyId]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for f in fields {
        for b in f.as_u32().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("tuple_{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_collapses_path_separator_then_maps_specials() {
        assert_eq!(sanitize("crate::mod::Foo"), "crate_mod_Foo");
        assert_eq!(sanitize("Foo<u32, [u8; 4]>"), "Foo_u32___u8__4__");
        assert_eq!(sanitize("already_ok_1"), "already_ok_1");
    }

    #[test]
    fn tuple_base_name_is_field_order_sensitive_and_stable() {
        let a = tuple_base_name(&[TyId::from_u32(1), TyId::from_u32(2)]);
        let b = tuple_base_name(&[TyId::from_u32(2), TyId::from_u32(1)]);
        assert_ne!(a, b);
        assert_eq!(a, tuple_base_name(&[TyId::from_u32(1), TyId::from_u32(2)]));
        assert!(a.starts_with("tuple_"));
    }
}
