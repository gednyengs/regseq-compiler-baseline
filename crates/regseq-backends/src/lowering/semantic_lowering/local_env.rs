//! Semantic local environment construction.
//!
//! Converts MIR `local_decls` into `Vec<SemanticLocal>` with proper
//! names, types, and origins. `self` is a real parameter local per the
//! accepted `SemanticBody` contract (plan.md Section 4.2).
//!
//! The local environment is built per-function before CFG walking.
//! During the walk, new derived locals may be introduced via
//! `allocate_derived()`.

use std::collections::{BTreeMap, HashSet};

use regseq_model::declaration::mir_body::{MirLocalDecl, MirRvalue};
use regseq_model::item::attributes::ItemAttribute;
use regseq_model::item::identity::ItemId;
use regseq_model::item::types::Item;
use regseq_model::semantic::local::{LocalId, LocalOrigin, SemanticLocal};
use regseq_model::semantic::types::SemanticType;

use super::type_lowering::lower_mir_ty;
use crate::lowering::hierarchy::ItemHierarchy;

// ---------------------------------------------------------------------------
// Local environment
// ---------------------------------------------------------------------------

/// Semantic local environment for a single function.
///
/// Built from MIR `local_decls` before CFG walking. Tracks the mapping
/// from MIR local indices to semantic `LocalId`s and whether each local
/// has been initialized (to distinguish `Let` from `Assign`).
pub struct LocalEnvironment {
    /// All semantic locals in declaration order.
    pub locals: Vec<SemanticLocal>,
    /// Parameter local IDs in signature order.
    pub params: Vec<LocalId>,
    /// Return type.
    pub return_type: SemanticType,
    /// MIR local index -> semantic LocalId.
    /// Missing entries are structural temps not materialized in semantic IR.
    mir_to_semantic: BTreeMap<u32, LocalId>,
    /// Set of locals that have been initialized (first Let emitted).
    initialized: HashSet<LocalId>,
    /// Next LocalId to allocate.
    next_id: u32,
}

impl LocalEnvironment {
    /// Build the local environment from MIR local declarations.
    ///
    /// `owner_id` is the `ItemId` of the owning type from `FunctionKind`.
    /// Used to determine the `self` type: `DriverData` for the driver
    /// struct, `Struct { id, name }` for non-driver helper types.
    ///
    /// `has_self_receiver` is true for methods with `&self` or `&mut self`.
    /// `param_names` are the authored parameter names from `FnParamDecl`.
    pub fn build(
        local_decls: &[MirLocalDecl],
        arg_count: usize,
        has_self_receiver: bool,
        param_names: &[String],
        owner_id: Option<ItemId>,
        hierarchy: &ItemHierarchy,
    ) -> Self {
        let mut env = Self {
            locals: Vec::new(),
            params: Vec::new(),
            return_type: SemanticType::Void,
            mir_to_semantic: BTreeMap::new(),
            initialized: HashSet::new(),
            next_id: 0,
        };

        if local_decls.is_empty() {
            return env;
        }

        // Local 0: return place.
        let return_ty = lower_mir_ty(&local_decls[0].ty, &hierarchy.cross_link, &hierarchy.store);
        env.return_type = return_ty.clone();
        let ret_id = env.push_local(SemanticLocal {
            id: LocalId(0),
            name: "_ret".into(),
            ty: return_ty,
            origin: LocalOrigin::ReturnPlace,
        });
        env.mir_to_semantic.insert(0, ret_id);

        // Local 1: self receiver (for methods).
        let param_start = if has_self_receiver {
            let self_ty = derive_self_type(owner_id, hierarchy);
            let self_id = env.push_local(SemanticLocal {
                id: LocalId(0),
                name: "self".into(),
                ty: self_ty,
                origin: LocalOrigin::Param { position: 0 },
            });
            env.mir_to_semantic.insert(1, self_id);
            env.params.push(self_id);
            2u32
        } else {
            1u32
        };

        // Remaining parameters.
        for (i, param_name) in param_names.iter().enumerate() {
            let local_idx = param_start + i as u32;
            let position = if has_self_receiver { i + 1 } else { i };
            let ty = local_decls
                .get(local_idx as usize)
                .map(|d| lower_mir_ty(&d.ty, &hierarchy.cross_link, &hierarchy.store))
                .unwrap_or(SemanticType::Opaque {
                    description: "missing param type".into(),
                });
            let param_id = env.push_local(SemanticLocal {
                id: LocalId(0),
                name: param_name.clone(),
                ty,
                origin: LocalOrigin::Param { position },
            });
            env.mir_to_semantic.insert(local_idx, param_id);
            env.params.push(param_id);
        }

        // Remaining locals (user-declared and compiler temps).
        let user_start = (arg_count as u32) + 1;
        for decl in local_decls.iter().skip(user_start as usize) {
            let name = decl.name.as_deref();
            if let Some(n) = name {
                if !is_mir_generated_name(n) {
                    // User-declared local with an authored name.
                    let ty = lower_mir_ty(&decl.ty, &hierarchy.cross_link, &hierarchy.store);
                    let local_id = env.push_local(SemanticLocal {
                        id: LocalId(0),
                        name: n.to_string(),
                        ty,
                        origin: LocalOrigin::Authored,
                    });
                    env.mir_to_semantic.insert(decl.index, local_id);
                }
                // MIR-generated _N names are handled lazily during the walk
                // via promote_temp_if_needed() when a value-producing
                // assignment is encountered.
            }
        }

        env
    }

    /// Push a new semantic local and return its ID.
    fn push_local(&mut self, mut local: SemanticLocal) -> LocalId {
        let id = LocalId(self.next_id);
        self.next_id += 1;
        local.id = id;
        self.locals.push(local);
        id
    }

    /// Look up the semantic LocalId for a MIR local index.
    pub fn mir_local(&self, mir_index: u32) -> Option<LocalId> {
        self.mir_to_semantic.get(&mir_index).copied()
    }

    /// Check if a MIR local index has a semantic local.
    pub fn has_mir_local(&self, mir_index: u32) -> bool {
        self.mir_to_semantic.contains_key(&mir_index)
    }

    /// Mark a local as initialized (first Let emitted).
    pub fn mark_initialized(&mut self, id: LocalId) {
        self.initialized.insert(id);
    }

    /// Check if a local has been initialized.
    pub fn is_initialized(&self, id: LocalId) -> bool {
        self.initialized.contains(&id)
    }

    /// Allocate a new derived local (for temps introduced during walking).
    pub fn allocate_derived(&mut self, name: &str, ty: SemanticType) -> LocalId {
        self.push_local(SemanticLocal {
            id: LocalId(0),
            name: name.to_string(),
            ty,
            origin: LocalOrigin::Derived,
        })
    }

    /// Promote a MIR temp local to a semantic derived local if it does not
    /// already have one. Returns the LocalId.
    ///
    /// Used when a value-producing assignment to a `_N`-named MIR temp is
    /// encountered during the CFG walk.
    pub fn promote_temp(&mut self, mir_index: u32, ty: SemanticType) -> LocalId {
        if let Some(id) = self.mir_to_semantic.get(&mir_index) {
            return *id;
        }
        let name = format!("t{mir_index}");
        let id = self.push_local(SemanticLocal {
            id: LocalId(0),
            name,
            ty,
            origin: LocalOrigin::Derived,
        });
        self.mir_to_semantic.insert(mir_index, id);
        id
    }

    /// Get a SemanticLocal by its LocalId.
    pub fn get_local(&self, id: LocalId) -> Option<&SemanticLocal> {
        self.locals.iter().find(|l| l.id == id)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive the semantic type for `self` from the owning type.
///
/// - Driver struct -> `SemanticType::DriverData`
/// - Non-driver struct -> `SemanticType::Struct { id, name }`
/// - Unknown/none -> `SemanticType::Opaque` (hard error upstream)
fn derive_self_type(owner_id: Option<ItemId>, hierarchy: &ItemHierarchy) -> SemanticType {
    let Some(id) = owner_id else {
        return SemanticType::Opaque {
            description: "self type: no owner".into(),
        };
    };

    let Some(item) = hierarchy.store.get(id) else {
        return SemanticType::Opaque {
            description: format!("self type: owner {:?} not found", id),
        };
    };

    match item {
        Item::Struct(s) => {
            if s.attributes.contains(&ItemAttribute::Driver) {
                SemanticType::DriverData
            } else {
                SemanticType::Struct {
                    id,
                    name: s.name.clone(),
                }
            }
        }
        _ => SemanticType::Opaque {
            description: format!("self type: owner {:?} is not a struct", id),
        },
    }
}

/// Check if a local name is a MIR-generated name (pattern: `_` followed by digits).
fn is_mir_generated_name(name: &str) -> bool {
    if let Some(rest) = name.strip_prefix('_') {
        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
    } else {
        false
    }
}

/// Check if a MIR rvalue is a value-producing computation suitable for
/// promoting a `_N`-named local to a semantic derived local.
pub fn is_value_producing_rvalue(rvalue: &MirRvalue) -> bool {
    match rvalue {
        MirRvalue::BinaryOp { .. } | MirRvalue::Cast { .. } | MirRvalue::UnaryOp { .. } => true,
        // Option carrier constructors (None, Some) are value-producing.
        MirRvalue::Aggregate {
            agg_kind: regseq_model::declaration::mir_body::MirAggregateKind::Adt { path, .. },
            ..
        } if path.contains("Option") => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_model::declaration::mir_body::{IntWidth, MirLocalDecl, MirMutability, MirTy};

    fn make_local(index: u32, ty: MirTy, name: Option<&str>) -> MirLocalDecl {
        MirLocalDecl {
            index,
            ty,
            mutability: MirMutability::Not,
            name: name.map(|s| s.to_string()),
        }
    }

    #[test]
    fn empty_decls_produces_empty_env() {
        let env = LocalEnvironment::build(&[], 0, false, &[], None, &empty_hierarchy());
        assert!(env.locals.is_empty());
        assert!(env.params.is_empty());
    }

    #[test]
    fn return_place_is_local_zero() {
        let decls = vec![make_local(
            0,
            MirTy::Int {
                signed: false,
                width: IntWidth::U32,
            },
            None,
        )];
        let env = LocalEnvironment::build(&decls, 0, false, &[], None, &empty_hierarchy());
        assert_eq!(env.locals.len(), 1);
        assert_eq!(env.locals[0].name, "_ret");
        assert_eq!(env.locals[0].origin, LocalOrigin::ReturnPlace);
        assert_eq!(env.return_type, SemanticType::U32);
    }

    #[test]
    fn self_is_params_zero_for_methods() {
        let decls = vec![
            make_local(0, MirTy::Tuple { elements: vec![] }, None),
            make_local(
                1,
                MirTy::Ref {
                    mutbl: true,
                    ty: Box::new(MirTy::Struct {
                        path: "Pl011FullSpec".into(),
                        fields: vec![],
                    }),
                },
                Some("self"),
            ),
        ];
        let env = LocalEnvironment::build(&decls, 1, true, &[], None, &empty_hierarchy());
        assert_eq!(env.params.len(), 1);
        let self_local = env.get_local(env.params[0]).unwrap();
        assert_eq!(self_local.name, "self");
        assert_eq!(self_local.origin, LocalOrigin::Param { position: 0 });
    }

    #[test]
    fn authored_params_have_correct_positions() {
        let decls = vec![
            make_local(0, MirTy::Tuple { elements: vec![] }, None),
            make_local(
                1,
                MirTy::Ref {
                    mutbl: true,
                    ty: Box::new(MirTy::Bool),
                },
                Some("self"),
            ),
            make_local(
                2,
                MirTy::Int {
                    signed: false,
                    width: IntWidth::U32,
                },
                Some("uart_clk_hz"),
            ),
            make_local(
                3,
                MirTy::Int {
                    signed: false,
                    width: IntWidth::U32,
                },
                Some("baud_rate"),
            ),
        ];
        let param_names = vec!["uart_clk_hz".into(), "baud_rate".into()];
        let env = LocalEnvironment::build(&decls, 3, true, &param_names, None, &empty_hierarchy());
        assert_eq!(env.params.len(), 3); // self + 2 params
        let p1 = env.get_local(env.params[1]).unwrap();
        assert_eq!(p1.name, "uart_clk_hz");
        assert_eq!(p1.origin, LocalOrigin::Param { position: 1 });
        let p2 = env.get_local(env.params[2]).unwrap();
        assert_eq!(p2.name, "baud_rate");
        assert_eq!(p2.origin, LocalOrigin::Param { position: 2 });
    }

    #[test]
    fn authored_locals_are_materialized() {
        let decls = vec![
            make_local(0, MirTy::Bool, None),
            make_local(1, MirTy::Bool, Some("handled")),
        ];
        let env = LocalEnvironment::build(&decls, 0, false, &[], None, &empty_hierarchy());
        // _ret + handled
        assert_eq!(env.locals.len(), 2);
        assert_eq!(env.locals[1].name, "handled");
        assert_eq!(env.locals[1].origin, LocalOrigin::Authored);
    }

    #[test]
    fn mir_generated_names_are_not_prematerialized() {
        let decls = vec![
            make_local(0, MirTy::Bool, None),
            make_local(
                1,
                MirTy::Int {
                    signed: false,
                    width: IntWidth::U32,
                },
                Some("_7"),
            ),
        ];
        let env = LocalEnvironment::build(&decls, 0, false, &[], None, &empty_hierarchy());
        // Only _ret; _7 is not prematerialized
        assert_eq!(env.locals.len(), 1);
        assert!(!env.has_mir_local(1));
    }

    #[test]
    fn promote_temp_creates_derived_local() {
        let decls = vec![make_local(0, MirTy::Bool, None)];
        let mut env = LocalEnvironment::build(&decls, 0, false, &[], None, &empty_hierarchy());
        let id = env.promote_temp(5, SemanticType::U32);
        let local = env.get_local(id).unwrap();
        assert_eq!(local.name, "t5");
        assert_eq!(local.origin, LocalOrigin::Derived);
        assert_eq!(local.ty, SemanticType::U32);
        // Second call returns the same ID.
        assert_eq!(env.promote_temp(5, SemanticType::U32), id);
    }

    fn empty_hierarchy() -> ItemHierarchy {
        ItemHierarchy {
            store: regseq_model::item::identity::ItemStore::new(),
            modules: vec![],
            cross_link: regseq_model::item::cross_link::CrossLinkTable::new(),
            interrupt_classes: vec![],
        }
    }
}
