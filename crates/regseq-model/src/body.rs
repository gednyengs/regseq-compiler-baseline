//! MIR body and local-declaration data shapes.
//!
//! `Body` is the function-level MIR container stored structurally on
//! `InstanceKind::Function`, not in a model-level body table. The
//! local layout convention is fixed by spec.md section 25.1:
//! `local_decls[0]` is the return place, `1..=arg_count` are
//! arguments, and the remaining locals are internal temporaries.

use serde::{Deserialize, Serialize};

use crate::basic_block::BasicBlock;
use crate::ids::{BlockId, LocalId, StringId, TyId};
use crate::index_vec::IndexVec;
use crate::source::Span;
use crate::ty::Mutability;

/// Function-level MIR container.
///
/// Bodies live structurally inside `InstanceKind::Function` per
/// spec.md sections 25.1, 25.4, and 25.5. There is no `BodyId`, no
/// model-level body table, no body-level function signature field,
/// and no body-to-item back-pointer.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Body {
    /// Body-local basic-block table, stored in reverse-postorder
    /// from `BlockId::START_BLOCK` (spec.md section 25.1).
    pub basic_blocks: IndexVec<BlockId, BasicBlock>,

    /// Body-local local declarations. Entry 0 is
    /// `LocalId::RETURN_PLACE`, entries `1..=arg_count` are
    /// positional arguments, and the rest are temporaries (spec.md
    /// section 25.1).
    pub local_decls: IndexVec<LocalId, LocalDecl>,

    /// Number of argument locals in `local_decls[1..=arg_count]`
    /// (spec.md section 25.1).
    pub arg_count: usize,

    /// Whole-body diagnostic location for body-level validation
    /// errors (spec.md section 25.1).
    pub span: Span,
}

impl Body {
    /// Returns the return-place local ID (`LocalId::RETURN_PLACE`).
    ///
    /// This helper records the layout convention from spec.md
    /// section 25.1.
    pub fn return_local(&self) -> LocalId {
        LocalId::RETURN_PLACE
    }

    /// Returns the positional argument locals in declaration order.
    ///
    /// Yields `LocalId(1)` through `LocalId(arg_count)` per spec.md
    /// section 25.1.
    pub fn arg_locals(&self) -> impl Iterator<Item = LocalId> {
        (1..=self.arg_count).map(|idx| LocalId::from_u32(idx as u32))
    }

    /// Returns the internal temporary locals in declaration order.
    ///
    /// Yields locals beginning at `arg_count + 1` through the end of
    /// `local_decls` per spec.md section 25.1.
    pub fn inner_locals(&self) -> impl Iterator<Item = LocalId> {
        ((self.arg_count + 1)..self.local_decls.len()).map(|idx| LocalId::from_u32(idx as u32))
    }

    /// Append `decl` to `local_decls` and return its freshly
    /// allocated `LocalId`.
    ///
    /// Allocation is positional: the returned `LocalId` equals the
    /// previous `local_decls.len()` (spec.md section 25.8).
    pub fn push_local(&mut self, decl: LocalDecl) -> LocalId {
        self.local_decls.push(decl)
    }

    /// Append a synthetic local declaration and return its freshly
    /// allocated `LocalId`.
    ///
    /// Synthetic locals are introduced by bridge collapse and
    /// rewrite passes. They default to `Mutability::Mut`,
    /// `Span::dummy()`, and `debug_name: None` per spec.md sections
    /// 25.2 and 25.8.
    pub fn push_synthetic_local(&mut self, ty: TyId) -> LocalId {
        self.push_local(LocalDecl {
            ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        })
    }
}

impl Default for Body {
    fn default() -> Self {
        Self {
            basic_blocks: IndexVec::new(),
            local_decls: IndexVec::new(),
            arg_count: 0,
            span: Span::dummy(),
        }
    }
}

/// Per-local declaration data.
///
/// This is the four-field local-declaration shape from spec.md
/// section 25.2. Synthetic locals carry `debug_name: None` and use
/// positional identity through their `LocalId`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalDecl {
    /// The local's interned type (spec.md section 25.2).
    pub ty: TyId,

    /// The local mutability marker from the source declaration or
    /// bridge synthesis rule (spec.md section 25.2).
    pub mutability: Mutability,

    /// Declaration location for source locals, or `Span::dummy()`
    /// for synthetic locals (spec.md section 25.2).
    pub span: Span,

    /// Source-level variable name, or `None` for compiler and bridge
    /// synthesized locals (spec.md section 25.2).
    pub debug_name: Option<StringId>,
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use super::{Body, LocalDecl};
    use crate::{
        BasicBlock, BlockId, IndexVec, LocalId, Mutability, SourceFileId, Span, StringId, TyId,
    };
    use crate::{Terminator, TerminatorKind};

    fn span(lo: u32, hi: u32) -> Span {
        Span {
            file: SourceFileId::SYNTHESIZED,
            lo,
            hi,
        }
    }

    fn local_decl(ty: u32, name: Option<u32>) -> LocalDecl {
        LocalDecl {
            ty: TyId::from_u32(ty),
            mutability: Mutability::Mut,
            span: span(ty, ty + 1),
            debug_name: name.map(StringId::from_u32),
        }
    }

    fn return_block() -> BasicBlock {
        BasicBlock {
            statements: Box::new([]),
            terminator: Terminator {
                kind: TerminatorKind::Return,
                span: span(90, 91),
            },
        }
    }

    fn empty_body() -> Body {
        Body {
            basic_blocks: IndexVec::new(),
            local_decls: IndexVec::new(),
            arg_count: 0,
            span: span(0, 1),
        }
    }

    fn body_with_counts(arg_count: usize, local_count: usize) -> Body {
        let mut body = empty_body();
        body.arg_count = arg_count;
        for idx in 0..local_count {
            body.push_local(local_decl(idx as u32, None));
        }
        body
    }

    fn hash_value<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    fn round_trip<T>(value: &T) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let json = serde_json::to_string(value).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn local_id_return_place_const_and_niche_are_pinned() {
        assert_eq!(LocalId::RETURN_PLACE, LocalId::from_u32(0));
        assert_eq!(core::mem::size_of::<LocalId>(), 4);
        assert_eq!(core::mem::size_of::<Option<LocalId>>(), 4);
    }

    #[test]
    fn local_decl_constructs_and_reads_fields() {
        let decl = LocalDecl {
            ty: TyId::from_u32(0),
            mutability: Mutability::Mut,
            span: span(10, 11),
            debug_name: Some(StringId::from_u32(7)),
        };

        assert_eq!(decl.ty, TyId::from_u32(0));
        assert_eq!(decl.mutability, Mutability::Mut);
        assert_eq!(decl.span, span(10, 11));
        assert_eq!(decl.debug_name, Some(StringId::from_u32(7)));
    }

    #[test]
    fn local_decl_synthetic_convention_uses_none_dummy_and_mut() {
        let decl = LocalDecl {
            ty: TyId::from_u32(3),
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        };

        assert_eq!(decl.ty, TyId::from_u32(3));
        assert_eq!(decl.mutability, Mutability::Mut);
        assert_eq!(decl.span, Span::dummy());
        assert_eq!(decl.debug_name, None);
    }

    #[test]
    fn local_decl_equality_and_hash_are_structural() {
        let left = local_decl(1, Some(7));
        let same = local_decl(1, Some(7));
        let different = local_decl(2, Some(7));

        assert_eq!(left, same);
        assert_eq!(hash_value(&left), hash_value(&same));
        assert_ne!(left, different);
    }

    #[test]
    fn body_constructs_empty_and_reads_fields() {
        let body = empty_body();

        assert!(body.basic_blocks.is_empty());
        assert!(body.local_decls.is_empty());
        assert_eq!(body.arg_count, 0);
        assert_eq!(body.span, span(0, 1));
    }

    #[test]
    fn body_default_is_empty_with_dummy_span() {
        let body = Body::default();

        assert!(body.basic_blocks.is_empty());
        assert!(body.local_decls.is_empty());
        assert_eq!(body.arg_count, 0);
        assert_eq!(body.span, Span::dummy());
    }

    #[test]
    fn body_constructs_with_block_and_return_local() {
        let mut blocks = IndexVec::new();
        let block = return_block();
        blocks.push(block.clone());
        let mut locals = IndexVec::new();
        let ret = local_decl(0, None);
        locals.push(ret.clone());
        let body = Body {
            basic_blocks: blocks,
            local_decls: locals,
            arg_count: 0,
            span: span(0, 1),
        };

        assert_eq!(body.basic_blocks[BlockId::START_BLOCK], block);
        assert_eq!(body.local_decls[LocalId::RETURN_PLACE], ret);
    }

    #[test]
    fn push_local_allocates_positionally_and_entries_are_indexable() {
        let mut body = empty_body();
        let first = local_decl(1, Some(7));
        let second = local_decl(2, Some(8));

        assert_eq!(body.push_local(first.clone()), LocalId::from_u32(0));
        assert_eq!(body.push_local(second.clone()), LocalId::from_u32(1));

        assert_eq!(body.local_decls.len(), 2);
        assert_eq!(body.local_decls[LocalId::from_u32(0)], first);
        assert_eq!(body.local_decls[LocalId::from_u32(1)], second);
    }

    #[test]
    fn push_synthetic_local_uses_defaults_and_allocates_positionally() {
        let mut body = empty_body();

        let first = body.push_synthetic_local(TyId::from_u32(11));
        let second = body.push_synthetic_local(TyId::from_u32(12));

        assert_eq!(first, LocalId::from_u32(0));
        assert_eq!(second, LocalId::from_u32(1));
        assert_eq!(body.local_decls[first].ty, TyId::from_u32(11));
        assert_eq!(body.local_decls[first].mutability, Mutability::Mut);
        assert_eq!(body.local_decls[first].span, Span::dummy());
        assert_eq!(body.local_decls[first].debug_name, None);
    }

    #[test]
    fn local_layout_helpers_return_expected_ids() {
        let empty_args = body_with_counts(0, 1);
        let three_args = body_with_counts(3, 4);
        let with_temps = body_with_counts(1, 5);

        assert_eq!(empty_args.return_local(), LocalId::RETURN_PLACE);
        assert!(empty_args.arg_locals().collect::<Vec<_>>().is_empty());
        assert_eq!(
            three_args.arg_locals().collect::<Vec<_>>(),
            vec![
                LocalId::from_u32(1),
                LocalId::from_u32(2),
                LocalId::from_u32(3)
            ]
        );
        assert_eq!(
            with_temps.inner_locals().collect::<Vec<_>>(),
            vec![
                LocalId::from_u32(2),
                LocalId::from_u32(3),
                LocalId::from_u32(4)
            ]
        );
    }

    #[test]
    fn body_equality_and_hash_are_structural() {
        let left = body_with_counts(1, 3);
        let same = body_with_counts(1, 3);
        let different = body_with_counts(0, 3);

        assert_eq!(left, same);
        assert_eq!(hash_value(&left), hash_value(&same));
        assert_ne!(left, different);
    }

    #[test]
    fn body_and_local_decl_serde_round_trip() {
        let empty = empty_body();
        let mut non_trivial = body_with_counts(0, 1);
        non_trivial.basic_blocks.push(return_block());
        let decl = local_decl(9, Some(4));

        assert_eq!(round_trip::<Body>(&empty), empty);
        assert_eq!(round_trip::<Body>(&non_trivial), non_trivial);
        assert_eq!(round_trip::<LocalDecl>(&decl), decl);
    }

    #[test]
    fn no_body_id_or_model_body_table_is_introduced() {
        let ids =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ids.rs")).unwrap();
        let model =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/model.rs")).unwrap();

        assert!(!ids.contains("pub struct BodyId"));
        assert!(!model.contains("bodies: IndexVec"));
    }

    #[test]
    fn body_struct_init_has_no_fn_sig_field() {
        // Struct initialization with exactly these four fields is the
        // compile-time check. Adding `fn_sig: ...` would fail to
        // compile.
        let body = Body {
            basic_blocks: IndexVec::new(),
            local_decls: IndexVec::new(),
            arg_count: 0,
            span: Span::dummy(),
        };

        assert_eq!(body.arg_count, 0);
    }
}
