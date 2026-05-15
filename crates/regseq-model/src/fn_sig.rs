//! Function signature schema and canonical byte encoding.

use serde::{Deserialize, Serialize};

use crate::args_key::encode_ty_with_tables;
use crate::const_interner::ConstInterner;
use crate::def_path::DefPathHash;
use crate::fn_sig_interner::FnSigInterner;
use crate::ids::TyId;
use crate::index_vec::IndexVec;
use crate::model::RegseqModel;
use crate::ty::{FnAbi, Safety};
use crate::ty_interner::TyInterner;

/// Canonical function signature schema.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FnSig {
    /// Positional argument types. Entries must not be `Ty::Never`;
    /// bridge and validator tasks enforce that invariant.
    pub inputs: Vec<TyId>,

    /// Return type. May be `Ty::Never` for divergent functions and
    /// may be the unit tuple shape for no-value returns.
    pub output: TyId,

    /// Function ABI. This is part of function-signature identity.
    pub abi: FnAbi,

    /// Whether the signature has a trailing C variadic argument list.
    ///
    /// Validator tasks enforce that this implies `abi == FnAbi::C`
    /// and at least one fixed input.
    pub c_variadic: bool,

    /// Function safety marker. This is part of function-signature
    /// identity.
    pub safety: Safety,
}

/// Returns the canonical byte stream for a function signature.
pub fn canonical_fn_sig(sig: &FnSig, model: &RegseqModel) -> Vec<u8> {
    let mut out = Vec::new();
    encode_fn_sig_with_tables(
        sig,
        &model.types,
        &model.sigs,
        Some(&model.consts),
        &model.item_def_paths,
        &mut out,
    );
    out
}

pub(crate) fn encode_fn_sig_with_tables(
    sig: &FnSig,
    types: &TyInterner,
    sigs: &FnSigInterner,
    consts: Option<&ConstInterner>,
    item_def_paths: &IndexVec<crate::ItemId, DefPathHash>,
    out: &mut Vec<u8>,
) {
    push_u32(out, sig.inputs.len());
    for input in &sig.inputs {
        encode_ty_with_tables(types, sigs, consts, item_def_paths, *input, out);
    }
    encode_ty_with_tables(types, sigs, consts, item_def_paths, sig.output, out);
    out.push(u8::from(sig.abi));
    out.push(u8::from(sig.c_variadic));
    out.push(u8::from(sig.safety));
}

fn push_u32(out: &mut Vec<u8>, value: usize) {
    let value = u32::try_from(value).expect("canonical length exceeds u32::MAX");
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::UNIT_LAYOUT;
    use crate::ty::{Mutability, Ty, UintKind};
    use crate::{StableCrateId, TyId};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn ty(value: u32) -> TyId {
        TyId::from_u32(value)
    }

    fn hash<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    fn base_sig() -> FnSig {
        FnSig {
            inputs: vec![ty(0), ty(1)],
            output: ty(0),
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        }
    }

    #[test]
    fn constructor_shapes_cover_schema_variants() {
        let unit_ty = ty(0);
        let u32_ty = ty(1);
        let ptr_ty = ty(2);
        let never_ty = ty(3);

        let empty = FnSig {
            inputs: vec![],
            output: unit_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        };
        let c_add = FnSig {
            inputs: vec![u32_ty, u32_ty],
            output: u32_ty,
            abi: FnAbi::C,
            c_variadic: false,
            safety: Safety::Safe,
        };
        let divergent = FnSig {
            inputs: vec![ptr_ty],
            output: never_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        };
        let variadic = FnSig {
            inputs: vec![u32_ty],
            output: unit_ty,
            abi: FnAbi::C,
            c_variadic: true,
            safety: Safety::Safe,
        };
        let unsafe_sig = FnSig {
            inputs: vec![],
            output: u32_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Unsafe,
        };

        assert!(empty.inputs.is_empty());
        assert_eq!(c_add.inputs, vec![u32_ty, u32_ty]);
        assert_eq!(divergent.output, never_ty);
        assert!(variadic.c_variadic);
        assert_eq!(unsafe_sig.safety, Safety::Unsafe);
    }

    #[test]
    fn equality_and_hash_cover_exact_schema_fields() {
        let sig = base_sig();
        let same = base_sig();
        assert_eq!(sig, same);
        assert_eq!(hash(&sig), hash(&same));

        let mut different_abi = sig.clone();
        different_abi.abi = FnAbi::C;
        assert_ne!(sig, different_abi);

        let mut different_variadic = sig.clone();
        different_variadic.c_variadic = true;
        assert_ne!(sig, different_variadic);

        let mut different_safety = sig.clone();
        different_safety.safety = Safety::Unsafe;
        assert_ne!(sig, different_safety);

        let mut different_inputs = sig.clone();
        different_inputs.inputs = vec![ty(1), ty(0)];
        assert_ne!(sig, different_inputs);

        let mut different_output = sig.clone();
        different_output.output = ty(1);
        assert_ne!(sig, different_output);
    }

    #[test]
    fn canonical_fn_sig_encodes_empty_rust_signature() {
        let mut model = RegseqModel::with_primary("primary", StableCrateId(1));
        let bool_ty = model.types.intern(Ty::Bool);
        let sig = FnSig {
            inputs: vec![],
            output: bool_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        };

        assert_eq!(canonical_fn_sig(&sig, &model), vec![0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn canonical_fn_sig_encodes_inputs_output_and_flags() {
        let mut model = RegseqModel::with_primary("primary", StableCrateId(1));
        let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
        let sig = FnSig {
            inputs: vec![u32_ty, u32_ty],
            output: u32_ty,
            abi: FnAbi::C,
            c_variadic: true,
            safety: Safety::Unsafe,
        };

        assert_eq!(
            canonical_fn_sig(&sig, &model),
            vec![2, 0, 0, 0, 3, 2, 3, 2, 3, 2, 1, 1, 1]
        );
    }

    #[test]
    fn canonical_fn_sig_allows_unit_and_pointer_shapes() {
        let mut model = RegseqModel::with_primary("primary", StableCrateId(1));
        let unit_ty = model.types.intern(Ty::Tuple {
            fields: vec![],
            layout: UNIT_LAYOUT,
        });
        let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
        let ptr_ty = model.types.intern(Ty::Ptr(u32_ty, Mutability::Not));
        let sig = FnSig {
            inputs: vec![ptr_ty],
            output: unit_ty,
            abi: FnAbi::C,
            c_variadic: false,
            safety: Safety::Safe,
        };

        assert_eq!(
            canonical_fn_sig(&sig, &model),
            vec![1, 0, 0, 0, 10, 3, 2, 0, 15, 0, 0, 0, 0, 1, 0, 0]
        );
    }
}
