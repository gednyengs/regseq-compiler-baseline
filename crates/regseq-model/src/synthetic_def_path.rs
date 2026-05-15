//! Synthetic `DefPath` segments and `DefPathHash` construction.

use sha2::{Digest, Sha256};

use crate::args_key::ArgsKey;
use crate::crate_info::StableCrateId;
use crate::def_path::{DefPathData, DefPathHash, DefPathSegment};
use crate::string_interner::StringInterner;
use crate::synthetic::{ClosureCallKind, ContentHash, PrincipalTraitKey, SyntheticItemKind};

/// Build the canonical synthetic `DefPath.segments` for a synthesised item.
///
/// The first segment is always `TypeNs("<regseq_synth>")`. Panics for
/// `SyntheticItemKind::None` and `SyntheticItemKind::ClosureCaptures`
/// because those items do not use synthetic segments.
pub fn synthetic_def_path_segments(
    kind: &SyntheticItemKind,
    strings: &mut StringInterner,
) -> Vec<DefPathSegment> {
    let mut texts = vec!["<regseq_synth>".to_owned()];
    match kind {
        SyntheticItemKind::None => panic!("SyntheticItemKind::None has no synthetic DefPath"),
        SyntheticItemKind::ClosureCaptures { .. } => {
            panic!("ClosureCaptures uses its rustc source-derived DefPath")
        }
        SyntheticItemKind::ClosureCall { closure, call_kind } => {
            texts.push("closure".to_owned());
            texts.push(def_path_hash_hex(*closure));
            texts.push(closure_call_kind_segment(*call_kind).to_owned());
        }
        SyntheticItemKind::ClosureDropGlue { closure } => {
            texts.push("closure".to_owned());
            texts.push(def_path_hash_hex(*closure));
            texts.push("drop_glue".to_owned());
        }
        SyntheticItemKind::DropGlue { self_ty } => {
            texts.push("drop_glue".to_owned());
            texts.push(args_key_hex(*self_ty));
        }
        SyntheticItemKind::VTableType { principal } => {
            push_principal_segments(&mut texts, "vtable_ty", principal);
        }
        SyntheticItemKind::VTableInstance { principal, self_ty } => {
            push_principal_segments(&mut texts, "vtable", principal);
            texts.push(args_key_hex(*self_ty));
        }
        SyntheticItemKind::VTableMethodShim {
            principal,
            self_ty,
            vtable_entry_index,
            method_def_path,
        } => {
            push_principal_segments(&mut texts, "vtable_method_shim", principal);
            texts.push(args_key_hex(*self_ty));
            texts.push(vtable_entry_index.to_string());
            texts.push(def_path_hash_hex(*method_def_path));
        }
        SyntheticItemKind::VTableDropShim { principal, self_ty } => {
            push_principal_segments(&mut texts, "vtable_drop_shim", principal);
            texts.push(args_key_hex(*self_ty));
        }
        SyntheticItemKind::ConstMemoryStatic { content_hash } => {
            texts.push("const_memory_static".to_owned());
            texts.push(content_hash_hex(content_hash));
        }
    }

    texts
        .into_iter()
        .map(|text| DefPathSegment {
            data: DefPathData::TypeNs(strings.intern(&text)),
            disambiguator: 0,
        })
        .collect()
}

/// Construct the canonical synthetic `DefPathHash` for a synthesised item.
///
/// The returned hash uses `StableCrateId(0)` as the reserved synthetic
/// domain marker. Panics for `SyntheticItemKind::None` and
/// `SyntheticItemKind::ClosureCaptures` because those items do not use
/// synthetic hashing.
pub fn synthetic_def_path_hash(kind: &SyntheticItemKind) -> DefPathHash {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"regseq-synthetic-item-v1\0");
    bytes.push(synthetic_kind_tag(kind));

    match kind {
        SyntheticItemKind::None => panic!("SyntheticItemKind::None has no synthetic DefPathHash"),
        SyntheticItemKind::ClosureCaptures { .. } => {
            panic!("ClosureCaptures uses its rustc source-derived DefPathHash")
        }
        SyntheticItemKind::ClosureCall { closure, call_kind } => {
            push_def_path_hash_bytes(&mut bytes, *closure);
            bytes.push(closure_call_kind_tag(*call_kind));
        }
        SyntheticItemKind::ClosureDropGlue { closure } => {
            push_def_path_hash_bytes(&mut bytes, *closure);
        }
        SyntheticItemKind::DropGlue { self_ty } => {
            push_args_key_bytes(&mut bytes, *self_ty);
        }
        SyntheticItemKind::VTableType { principal } => {
            push_principal_bytes(&mut bytes, principal);
        }
        SyntheticItemKind::VTableInstance { principal, self_ty }
        | SyntheticItemKind::VTableDropShim { principal, self_ty } => {
            push_principal_bytes(&mut bytes, principal);
            push_args_key_bytes(&mut bytes, *self_ty);
        }
        SyntheticItemKind::VTableMethodShim {
            principal,
            self_ty,
            vtable_entry_index,
            method_def_path,
        } => {
            push_principal_bytes(&mut bytes, principal);
            push_args_key_bytes(&mut bytes, *self_ty);
            bytes.extend_from_slice(&vtable_entry_index.to_le_bytes());
            push_def_path_hash_bytes(&mut bytes, *method_def_path);
        }
        SyntheticItemKind::ConstMemoryStatic { content_hash } => {
            push_content_hash_bytes(&mut bytes, content_hash);
        }
    }

    let digest = Sha256::digest(&bytes);
    let mut local = [0u8; 8];
    local.copy_from_slice(&digest[..8]);
    DefPathHash {
        stable_crate_id: StableCrateId(0),
        local_hash: u64::from_be_bytes(local),
    }
}

fn push_principal_segments(
    texts: &mut Vec<String>,
    family: &'static str,
    principal: &PrincipalTraitKey,
) {
    texts.push(family.to_owned());
    texts.push(def_path_hash_hex(principal.trait_path));
    texts.push(args_key_hex(principal.trait_args));
    texts.push(projection_bindings_hex(&principal.projection_bindings));
}

fn synthetic_kind_tag(kind: &SyntheticItemKind) -> u8 {
    match kind {
        SyntheticItemKind::None => 0,
        SyntheticItemKind::ClosureCaptures { .. } => 1,
        SyntheticItemKind::ClosureCall { .. } => 2,
        SyntheticItemKind::ClosureDropGlue { .. } => 3,
        SyntheticItemKind::DropGlue { .. } => 4,
        SyntheticItemKind::VTableType { .. } => 5,
        SyntheticItemKind::VTableInstance { .. } => 6,
        SyntheticItemKind::VTableMethodShim { .. } => 7,
        SyntheticItemKind::VTableDropShim { .. } => 8,
        SyntheticItemKind::ConstMemoryStatic { .. } => 9,
    }
}

fn closure_call_kind_tag(kind: ClosureCallKind) -> u8 {
    match kind {
        ClosureCallKind::Call => 0,
        ClosureCallKind::CallMut => 1,
        ClosureCallKind::CallOnce => 2,
    }
}

fn closure_call_kind_segment(kind: ClosureCallKind) -> &'static str {
    match kind {
        ClosureCallKind::Call => "call",
        ClosureCallKind::CallMut => "call_mut",
        ClosureCallKind::CallOnce => "call_once",
    }
}

fn push_principal_bytes(out: &mut Vec<u8>, principal: &PrincipalTraitKey) {
    push_def_path_hash_bytes(out, principal.trait_path);
    push_args_key_bytes(out, principal.trait_args);
    push_projection_bindings_bytes(out, &principal.projection_bindings);
}

fn push_def_path_hash_bytes(out: &mut Vec<u8>, hash: DefPathHash) {
    out.extend_from_slice(&hash.stable_crate_id.0.to_le_bytes());
    out.extend_from_slice(&hash.local_hash.to_le_bytes());
}

fn push_args_key_bytes(out: &mut Vec<u8>, key: ArgsKey) {
    out.extend_from_slice(&key.0.to_be_bytes());
}

fn push_projection_bindings_bytes(out: &mut Vec<u8>, key: &crate::ProjectionBindingsKey) {
    out.extend_from_slice(&key.0.to_be_bytes());
}

fn push_content_hash_bytes(out: &mut Vec<u8>, key: &ContentHash) {
    out.extend_from_slice(&key.0.to_be_bytes());
}

fn def_path_hash_hex(hash: DefPathHash) -> String {
    let mut bytes = Vec::with_capacity(16);
    push_def_path_hash_bytes(&mut bytes, hash);
    bytes_to_hex(&bytes)
}

fn args_key_hex(key: ArgsKey) -> String {
    bytes_to_hex(&key.0.to_be_bytes())
}

fn projection_bindings_hex(key: &crate::ProjectionBindingsKey) -> String {
    bytes_to_hex(&key.0.to_be_bytes())
}

fn content_hash_hex(key: &ContentHash) -> String {
    bytes_to_hex(&key.0.to_be_bytes())
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::def_path::DefPathData;
    use crate::{
        ArgsKey, ClosureCallKind, ContentHash, PrincipalTraitKey, ProjectionBindingsKey,
        StableCrateId,
    };

    fn def_hash(stable_crate_id: u64, local_hash: u64) -> DefPathHash {
        DefPathHash {
            stable_crate_id: StableCrateId(stable_crate_id),
            local_hash,
        }
    }

    fn principal() -> PrincipalTraitKey {
        PrincipalTraitKey {
            trait_path: def_hash(0x711A17, 0x7A17),
            trait_args: ArgsKey(0x1111_2222_3333_4444_5555_6666_7777_8888),
            projection_bindings: ProjectionBindingsKey(0x9999_aaaa_bbbb_cccc_dddd_eeee_ffff_0000),
        }
    }

    fn self_ty() -> ArgsKey {
        ArgsKey(0x1234_5678_9abc_def0_0102_0304_0506_0708)
    }

    fn synthetic_kinds() -> Vec<SyntheticItemKind> {
        let principal = principal();
        vec![
            SyntheticItemKind::ClosureCall {
                closure: def_hash(0xC105, 0xA11CE),
                call_kind: ClosureCallKind::Call,
            },
            SyntheticItemKind::ClosureDropGlue {
                closure: def_hash(0xC105, 0xA11CE),
            },
            SyntheticItemKind::VTableType {
                principal: principal.clone(),
            },
            SyntheticItemKind::VTableInstance {
                principal: principal.clone(),
                self_ty: self_ty(),
            },
            SyntheticItemKind::VTableMethodShim {
                principal: principal.clone(),
                self_ty: self_ty(),
                vtable_entry_index: 4,
                method_def_path: def_hash(0xCA11, 0xA),
            },
            SyntheticItemKind::VTableDropShim {
                principal,
                self_ty: self_ty(),
            },
            SyntheticItemKind::ConstMemoryStatic {
                content_hash: ContentHash(0xCAFE),
            },
        ]
    }

    fn segment_texts(kind: &SyntheticItemKind) -> Vec<String> {
        let mut strings = StringInterner::new();
        synthetic_def_path_segments(kind, &mut strings)
            .iter()
            .map(|segment| match segment.data {
                DefPathData::TypeNs(id) => strings.resolve(id).to_owned(),
                _ => panic!("synthetic segment was not TypeNs"),
            })
            .collect()
    }

    #[test]
    fn synthetic_segments_start_with_reserved_domain_separator() {
        for kind in synthetic_kinds() {
            let texts = segment_texts(&kind);
            assert_eq!(texts.first().unwrap(), "<regseq_synth>");
        }
    }

    #[test]
    fn synthetic_segments_have_family_specific_counts() {
        let principal = principal();
        let cases = [
            (
                SyntheticItemKind::ClosureCall {
                    closure: def_hash(0xC105, 0xA11CE),
                    call_kind: ClosureCallKind::Call,
                },
                4,
            ),
            (
                SyntheticItemKind::ClosureDropGlue {
                    closure: def_hash(0xC105, 0xA11CE),
                },
                4,
            ),
            (
                SyntheticItemKind::VTableType {
                    principal: principal.clone(),
                },
                5,
            ),
            (
                SyntheticItemKind::VTableInstance {
                    principal: principal.clone(),
                    self_ty: self_ty(),
                },
                6,
            ),
            (
                SyntheticItemKind::VTableMethodShim {
                    principal: principal.clone(),
                    self_ty: self_ty(),
                    vtable_entry_index: 4,
                    method_def_path: def_hash(0xCA11, 0xA),
                },
                8,
            ),
            (
                SyntheticItemKind::VTableDropShim {
                    principal,
                    self_ty: self_ty(),
                },
                6,
            ),
            (
                SyntheticItemKind::ConstMemoryStatic {
                    content_hash: ContentHash(0xCAFE),
                },
                3,
            ),
        ];

        for (kind, expected_len) in cases {
            assert_eq!(segment_texts(&kind).len(), expected_len);
        }
    }

    #[test]
    #[should_panic]
    fn synthetic_segments_reject_none() {
        let mut strings = StringInterner::new();
        let _ = synthetic_def_path_segments(&SyntheticItemKind::None, &mut strings);
    }

    #[test]
    #[should_panic]
    fn synthetic_segments_reject_closure_captures() {
        let mut strings = StringInterner::new();
        let _ = synthetic_def_path_segments(
            &SyntheticItemKind::ClosureCaptures {
                closure: def_hash(0xC105, 0xA11CE),
            },
            &mut strings,
        );
    }

    #[test]
    fn synthetic_hashes_use_reserved_stable_crate_id_zero() {
        for kind in synthetic_kinds() {
            assert_eq!(
                synthetic_def_path_hash(&kind).stable_crate_id,
                StableCrateId(0)
            );
        }
    }

    #[test]
    fn synthetic_hashes_are_deterministic() {
        for kind in synthetic_kinds() {
            assert_eq!(
                synthetic_def_path_hash(&kind),
                synthetic_def_path_hash(&kind)
            );
        }
    }

    #[test]
    fn synthetic_hash_variant_tags_domain_separate_families() {
        let principal = principal();
        let instance = SyntheticItemKind::VTableInstance {
            principal: principal.clone(),
            self_ty: self_ty(),
        };
        let drop_shim = SyntheticItemKind::VTableDropShim {
            principal,
            self_ty: self_ty(),
        };

        assert_ne!(
            synthetic_def_path_hash(&instance).local_hash,
            synthetic_def_path_hash(&drop_shim).local_hash
        );
    }

    #[test]
    fn synthetic_hash_closure_call_kind_discriminant_is_load_bearing() {
        let closure = def_hash(0xC105, 0xA11CE);
        let hashes: HashSet<u64> = [
            ClosureCallKind::Call,
            ClosureCallKind::CallMut,
            ClosureCallKind::CallOnce,
        ]
        .into_iter()
        .map(|call_kind| {
            synthetic_def_path_hash(&SyntheticItemKind::ClosureCall { closure, call_kind })
                .local_hash
        })
        .collect();

        assert_eq!(hashes.len(), 3);
    }

    #[test]
    #[should_panic]
    fn synthetic_hash_rejects_none() {
        let _ = synthetic_def_path_hash(&SyntheticItemKind::None);
    }

    #[test]
    #[should_panic]
    fn synthetic_hash_rejects_closure_captures() {
        let _ = synthetic_def_path_hash(&SyntheticItemKind::ClosureCaptures {
            closure: def_hash(0xC105, 0xA11CE),
        });
    }
}
