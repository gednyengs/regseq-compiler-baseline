//! The generic-C backend driver: the `generic-c-backend.md` section 15
//! five-bank single-translation-unit assembly.
//!
//! Task b shipped the model-independent scaffolding (the bank-ordering
//! driver + bank 1, the [`prelude`]). Task c1 fills banks 2-3: the
//! type-system foundation -- the `Ty`->C spelling ([`types`]), the D1 type
//! naming ([`naming`] + [`NameTable`]), the section 14 / D10 field-offset
//! aggregate emitter ([`layout_emit`]), the section 3.2 fat-pointer struct
//! shapes, and the section 15 by-value-containment discovery + topo-sort
//! that orders the definitions. Banks 4-5 (prototypes, `extern` statics,
//! function bodies, static initializers) remain task-d hooks.
//!
//! Scope (task c1): emits compilable C type definitions for sized,
//! non-enum, non-`dyn` aggregates (struct / union / tuple / closure /
//! array / slice / primitives), each with `_Static_assert` layout guards.
//! Enum *bodies* are the task-c2 hook (`InstanceKind::Enum` is forward-
//! declared + spelled but its body is skipped); `dyn` vtable construction +
//! virtual dispatch are deferred (only the `trait_obj` shape + a VTable
//! forward-decl ship here).

use std::collections::{HashMap, HashSet};

use regseq_model::{InstanceKind, Mutability, Provenance, RegseqModel, Ty, TyId};

pub mod prelude;

mod layout_emit;
mod naming;
mod types;

/// A node in the section 15 by-value-containment graph: an aggregate to
/// emit (a definition or, for enums, a forward-decl only) or a synthesized
/// fat-pointer struct (section 3.2). The fat-pointer variants are *not*
/// `Ty` aggregates -- they are derived from a `Ref` / `Ptr` to an unsized
/// pointee -- so they carry the data needed to re-derive their shape.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Node {
    /// A nominal or structural aggregate: `Ty::Struct` / `Union` / `Enum`
    /// / `Tuple` / `Closure`.
    Agg(TyId),
    /// The section 3.2 `rust_slice_<T>` fat struct for a `Ref` / `Ptr` to
    /// `Slice(elem)`. `const_data` records whether the `data` slot is
    /// `const` (so distinct mutability flavors of the same element are
    /// distinct nodes, disambiguated by the `_clash_<n>` net).
    Slice {
        /// Slice element type.
        elem: TyId,
        /// Whether the `data` pointer is `const`.
        const_data: bool,
    },
    /// The section 3.2 `trait_obj_<Trait>` fat struct for a `Ref` / `Ptr`
    /// to a `Dynamic` pointee, identified by the `Dynamic` `TyId`.
    TraitObj {
        /// The `Ty::Dynamic` pointee `TyId`.
        dynamic: TyId,
    },
}

/// The D1 name table: a pure function of the model that maps every
/// discovered [`Node`] to its final, collision-free C identifier. Base
/// names are derived (`naming::aggregate_base_name`, the fat-struct
/// shapes); the `_clash_<n>` pass over the assembled table is the safety
/// net (a no-op for a well-formed model whose `(DefPath, ArgsKey)` keys
/// are unique).
struct NameTable {
    names: HashMap<Node, String>,
}

impl NameTable {
    /// The final C identifier assigned to `node`. Panics if `node` was
    /// not part of the discovered set the table was built from.
    fn name(&self, node: &Node) -> &str {
        self.names
            .get(node)
            .expect("NameTable queried for an undiscovered node")
    }

    /// The final C identifier for an aggregate `TyId`.
    fn agg_name(&self, ty: TyId) -> &str {
        self.name(&Node::Agg(ty))
    }
}

/// Whether the `data` pointer of the fat slice struct for a `Ref` / `Ptr`
/// `ref_ty` is `const` (section 3.1 / section 3.2 table).
fn slice_data_const(model: &RegseqModel, ref_ty: TyId) -> bool {
    match &model.types[ref_ty] {
        Ty::Ref(pointee, Provenance::SharedBorrow) => !model.contains_unsafe_cell(*pointee),
        Ty::Ref(_, Provenance::UniqueBorrow) => false,
        Ty::Ptr(_, Mutability::Not) => true,
        Ty::Ptr(_, Mutability::Mut) => false,
        _ => true,
    }
}

/// Lower a post-mono [`RegseqModel`] to a single generic-C translation
/// unit in `generic-c-backend.md` section 15 five-bank order.
///
/// Banks: (1) the prelude + helper block (model-independent), (2) type
/// forward-decls, (3) type definitions (topologically sorted by by-value
/// containment), (4) declarations, (5) definitions (banks 4-5 are task-d
/// hooks, empty for now). The output is a *pure function of the model* (no
/// wall-clock / iteration nondeterminism) so the snapshot oracle's
/// byte-stability holds.
///
/// Returns [`EmitError::ContainmentCycle`] if the section 15 by-value
/// containment graph has a cycle (impossible for sized Rust types, so it
/// signals model corruption); otherwise `Ok`.
pub fn emit_generic_c(model: &RegseqModel) -> Result<String, EmitError> {
    let discovered = discover(model);
    let names = build_name_table(model, &discovered);
    let topo = topo_sort(model, &discovered)?;

    let mut out = String::new();
    out.push_str(prelude::PRELUDE);
    out.push_str(&prelude::helper_block());
    out.push_str(&bank2_type_forward_decls(model, &names, &discovered));
    out.push_str(&bank3_type_definitions(model, &names, &topo));
    out.push_str(&bank4_declarations(model));
    out.push_str(&bank5_definitions(model));
    Ok(out)
}

/// The by-value-containment edges out of `node` (section 15 edge set):
/// edges through `FieldLayout.ty` (struct / union / tuple / enum-payload /
/// closure-capture fields) + the element of a reached `Ty::Array`; a
/// field that is a `Ref` / `Ptr` to an unsized pointee edges to its fat
/// struct (`rust_slice_<T>` / `trait_obj_<Trait>`; `rust_str` is bank 1,
/// always complete). No edge through `Ref` / `Ptr` to a sized pointee,
/// `FnPtr`, or `FnDef`. Fat-struct nodes hold pointers, so they have no
/// onward by-value edge.
fn node_edges(model: &RegseqModel, node: &Node) -> Vec<Node> {
    match node {
        Node::Slice { .. } | Node::TraitObj { .. } => Vec::new(),
        Node::Agg(ty) => raw_field_tys(model, *ty)
            .into_iter()
            .filter(|field_ty| model.size_align_of(*field_ty).size > 0)
            .filter_map(|field_ty| containment_target(model, field_ty))
            .collect(),
    }
}

/// The raw field types of an aggregate `ty` (source / MIR order), across
/// the struct / union / tuple / closure / enum shapes. Enums contribute
/// every variant's payload fields (the section 15 edge set includes enum
/// payloads, even though task c1 emits no enum body).
fn raw_field_tys(model: &RegseqModel, ty: TyId) -> Vec<TyId> {
    use regseq_model::{EnumShape, LayoutKind};
    let layout = match &model.types[ty] {
        Ty::Struct { layout, .. } | Ty::Union { layout, .. } | Ty::Tuple { layout, .. } => layout,
        Ty::Enum { layout, .. } => layout,
        Ty::Closure(item, args) => {
            match model
                .types
                .iter()
                .find_map(|(_, candidate)| match candidate {
                    Ty::Struct {
                        item: s_item,
                        args: s_args,
                        layout,
                    } if s_item == item && s_args == args => Some(layout),
                    _ => None,
                }) {
                Some(layout) => layout,
                None => return Vec::new(),
            }
        }
        _ => return Vec::new(),
    };
    match &layout.kind {
        LayoutKind::Struct { fields } | LayoutKind::Union { fields } => {
            fields.iter().map(|f| f.ty).collect()
        }
        LayoutKind::Enum { shape } => match shape {
            EnumShape::Empty => Vec::new(),
            EnumShape::Single { variant, .. } => variant.fields.iter().map(|f| f.ty).collect(),
            EnumShape::Multiple { variants, .. } => variants
                .iter()
                .flat_map(|v| v.fields.iter().map(|f| f.ty))
                .collect(),
        },
    }
}

/// The containment-edge target of a field type `field_ty` (section 15):
/// an aggregate edges to its own node; an array edges to its (recursively
/// resolved) element; a `Ref` / `Ptr` to an unsized pointee edges to its
/// fat struct; everything else (sized pointer, scalar, `FnPtr`, `FnDef`)
/// has no by-value edge.
fn containment_target(model: &RegseqModel, field_ty: TyId) -> Option<Node> {
    match &model.types[field_ty] {
        Ty::Struct { .. }
        | Ty::Union { .. }
        | Ty::Enum { .. }
        | Ty::Tuple { .. }
        | Ty::Closure(_, _) => Some(Node::Agg(field_ty)),
        Ty::Array(elem, _) => containment_target(model, *elem),
        Ty::Ref(_, _) | Ty::Ptr(_, _) => unsized_pointee_node(model, field_ty),
        _ => None,
    }
}

/// If `ref_ty` is a `Ref` / `Ptr` to an unsized pointee, its fat-struct
/// node: `Slice` for `Slice(elem)`, `TraitObj` for `Dynamic`, `None` for
/// `Str` (the monomorphic `rust_str` lives in bank 1) or a sized pointee.
fn unsized_pointee_node(model: &RegseqModel, ref_ty: TyId) -> Option<Node> {
    let pointee = match &model.types[ref_ty] {
        Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
        _ => return None,
    };
    match &model.types[pointee] {
        Ty::Slice(elem) => Some(Node::Slice {
            elem: *elem,
            const_data: slice_data_const(model, ref_ty),
        }),
        Ty::Dynamic { .. } => Some(Node::TraitObj { dynamic: pointee }),
        _ => None,
    }
}

/// Discover every aggregate / fat-pointer node reachable by by-value
/// containment from the nominal roots (every `InstanceKind::Struct` /
/// `Union` / `Enum` instance). Returns first-seen order, a deterministic
/// function of `InstanceId` order + the section 15 edge set. (Task c1's
/// roots are nominal-containment only: structural aggregates used *only*
/// in a signature / local / static type are NOT roots here -- task d must
/// widen the roots when it adds banks 4-5.)
fn discover(model: &RegseqModel) -> Vec<Node> {
    let mut seen: HashSet<Node> = HashSet::new();
    let mut order: Vec<Node> = Vec::new();
    for instance in model.instances.iter() {
        let root_ty = match instance.kind {
            InstanceKind::Struct { layout }
            | InstanceKind::Enum { layout }
            | InstanceKind::Union { layout } => layout,
            _ => continue,
        };
        dfs_discover(model, Node::Agg(root_ty), &mut seen, &mut order);
    }
    order
}

/// Pre-order DFS collecting nodes into first-seen `order`.
fn dfs_discover(model: &RegseqModel, node: Node, seen: &mut HashSet<Node>, order: &mut Vec<Node>) {
    if !seen.insert(node.clone()) {
        return;
    }
    order.push(node.clone());
    for child in node_edges(model, &node) {
        dfs_discover(model, child, seen, order);
    }
}

/// Topo-sort visit mark: a node mid-DFS (`InProgress`) re-entered means a
/// by-value cycle; a finished node (`Done`) is skipped.
#[derive(PartialEq, Clone, Copy)]
enum Mark {
    InProgress,
    Done,
}

/// Topologically sort the discovered nodes so a node is emitted after
/// every node it contains by value (section 15). Post-order DFS over the
/// discovery order (deterministic tie-break); a re-entered in-progress
/// node is a by-value cycle -> [`EmitError::ContainmentCycle`].
fn topo_sort(model: &RegseqModel, nodes: &[Node]) -> Result<Vec<Node>, EmitError> {
    let mut state: HashMap<Node, Mark> = HashMap::new();
    let mut out: Vec<Node> = Vec::new();
    for node in nodes {
        topo_visit(model, node, &mut state, &mut out)?;
    }
    Ok(out)
}

/// Post-order DFS appending `node` to `out` after every node it contains.
fn topo_visit(
    model: &RegseqModel,
    node: &Node,
    state: &mut HashMap<Node, Mark>,
    out: &mut Vec<Node>,
) -> Result<(), EmitError> {
    match state.get(node) {
        Some(Mark::Done) => return Ok(()),
        Some(Mark::InProgress) => return Err(EmitError::ContainmentCycle),
        None => {}
    }
    state.insert(node.clone(), Mark::InProgress);
    for child in node_edges(model, node) {
        topo_visit(model, &child, state, out)?;
    }
    state.insert(node.clone(), Mark::Done);
    out.push(node.clone());
    Ok(())
}

/// Build the D1 [`NameTable`] over the discovered nodes (discovery order),
/// applying the `_clash_<n>` disambiguation pass.
fn build_name_table(model: &RegseqModel, nodes: &[Node]) -> NameTable {
    let mut names: HashMap<Node, String> = HashMap::new();
    let mut used: HashSet<String> = HashSet::new();
    for node in nodes {
        let base = base_name(model, node);
        let mut candidate = base.clone();
        let mut n = 1usize;
        while used.contains(&candidate) {
            candidate = format!("{base}_clash_{n}");
            n += 1;
        }
        used.insert(candidate.clone());
        names.insert(node.clone(), candidate);
    }
    NameTable { names }
}

/// The derived base name (pre-`_clash_<n>`) for a node.
fn base_name(model: &RegseqModel, node: &Node) -> String {
    match node {
        Node::Agg(ty) => naming::aggregate_base_name(model, *ty),
        Node::Slice { elem, .. } => format!("rust_slice_{}", elem_label(model, *elem)),
        Node::TraitObj { dynamic } => format!("trait_obj_{}", trait_label(model, *dynamic)),
    }
}

/// A stable label for a fat-slice element type, used in `rust_slice_<T>`.
/// Scalars use their C spelling; aggregates use their derived base name;
/// other forms fall back to a deterministic token. Computed without the
/// [`NameTable`] (it is consumed while building the table).
fn elem_label(model: &RegseqModel, elem: TyId) -> String {
    match &model.types[elem] {
        Ty::Struct { .. }
        | Ty::Union { .. }
        | Ty::Enum { .. }
        | Ty::Tuple { .. }
        | Ty::Closure(_, _) => naming::sanitize(&naming::aggregate_base_name(model, elem)),
        Ty::Array(inner, _) => format!("arr_{}", elem_label(model, *inner)),
        Ty::Ref(_, _) | Ty::Ptr(_, _) => "ptr".to_string(),
        _ => naming::sanitize(&scalar_label(model, elem)),
    }
}

/// The C spelling of a scalar / simple type for use in a derived label,
/// without needing the [`NameTable`]. Aggregates are handled by the
/// caller; here only scalars and a few simple forms occur.
fn scalar_label(model: &RegseqModel, ty: TyId) -> String {
    use regseq_model::{FloatKind, IntKind, UintKind};
    match &model.types[ty] {
        Ty::Bool => "_Bool".to_string(),
        Ty::Char => "uint32_t".to_string(),
        Ty::Int(IntKind::I8) => "int8_t".to_string(),
        Ty::Int(IntKind::I16) => "int16_t".to_string(),
        Ty::Int(IntKind::I32) => "int32_t".to_string(),
        Ty::Int(IntKind::I64) => "int64_t".to_string(),
        Ty::Int(IntKind::I128) => "__int128".to_string(),
        Ty::Int(IntKind::Isize) => "intptr_t".to_string(),
        Ty::Uint(UintKind::U8) => "uint8_t".to_string(),
        Ty::Uint(UintKind::U16) => "uint16_t".to_string(),
        Ty::Uint(UintKind::U32) => "uint32_t".to_string(),
        Ty::Uint(UintKind::U64) => "uint64_t".to_string(),
        Ty::Uint(UintKind::U128) => "__uint128_t".to_string(),
        Ty::Uint(UintKind::Usize) => "size_t".to_string(),
        Ty::Float(FloatKind::F16) => "_Float16".to_string(),
        Ty::Float(FloatKind::F32) => "float".to_string(),
        Ty::Float(FloatKind::F64) => "double".to_string(),
        Ty::Float(FloatKind::F128) => "_Float128".to_string(),
        _ => format!("ty{}", ty.as_u32()),
    }
}

/// The derived trait label for a `trait_obj_<Trait>` / `<Trait>_VTable`
/// name, from the `Ty::Dynamic`'s synthesized `vtable_ty`. A nominal
/// vtable type uses its derived base name; otherwise a deterministic
/// fallback. (`dyn` is deferred per scope decision 2; this only needs to
/// be deterministic and consistent across the shape + its VTable
/// forward-decl.)
fn trait_label(model: &RegseqModel, dynamic: TyId) -> String {
    let vtable_ty = match &model.types[dynamic] {
        Ty::Dynamic { vtable_ty, .. } => *vtable_ty,
        _ => return format!("dyn_{}", dynamic.as_u32()),
    };
    match &model.types[vtable_ty] {
        Ty::Struct { .. }
        | Ty::Union { .. }
        | Ty::Enum { .. }
        | Ty::Tuple { .. }
        | Ty::Closure(_, _) => naming::aggregate_base_name(model, vtable_ty),
        _ => format!("dyn_{}", vtable_ty.as_u32()),
    }
}

/// Bank 2 (section 15): tag forward-decls for every discovered aggregate
/// in F4 (discovery) order -- `struct <T>;` for struct / tuple / closure /
/// enum / fat structs, `union <T>;` for unions. Each `trait_obj_<Trait>`
/// also forward-decls its `<Trait>_VTable`.
fn bank2_type_forward_decls(model: &RegseqModel, names: &NameTable, discovered: &[Node]) -> String {
    let mut out = String::new();
    if discovered.is_empty() {
        return out;
    }
    out.push_str(
        "\n/* ---------------------------------------------------------------------------\n\
         \x20* Bank 2 (section 15): aggregate tag forward-decls.\n\
         \x20* --------------------------------------------------------------------------- */\n",
    );
    for node in discovered {
        let name = names.name(node);
        match node {
            Node::Agg(ty) if matches!(model.types[*ty], Ty::Union { .. }) => {
                out.push_str(&format!("union {name};\n"));
            }
            Node::Agg(_) | Node::Slice { .. } => {
                out.push_str(&format!("struct {name};\n"));
            }
            Node::TraitObj { dynamic } => {
                out.push_str(&format!(
                    "struct {}_VTable;\n",
                    trait_label(model, *dynamic)
                ));
                out.push_str(&format!("struct {name};\n"));
            }
        }
    }
    out
}

/// Bank 3 (section 15): full `struct` / `union` bodies, topologically
/// sorted by by-value containment. Non-enum aggregates go through the
/// section 14 emitter; `InstanceKind::Enum` -> the task-c2 hook (no body;
/// the bank-2 forward-decl stands); fat structs emit their section 3.2
/// shape.
fn bank3_type_definitions(model: &RegseqModel, names: &NameTable, topo: &[Node]) -> String {
    let mut out = String::new();
    if topo.is_empty() {
        return out;
    }
    out.push_str(
        "\n/* ---------------------------------------------------------------------------\n\
         \x20* Bank 3 (section 15): aggregate definitions + section 14 layout guards.\n\
         \x20* --------------------------------------------------------------------------- */\n",
    );
    for node in topo {
        let name = names.name(node);
        match node {
            Node::Agg(ty) => {
                if matches!(model.types[*ty], Ty::Enum { .. }) {
                    // Task-c2 hook (section 13 / D9): the enum body reuses
                    // the section 14 primitive; the bank-2 forward-decl
                    // stands until then.
                    out.push_str(&format!("/* TODO(task c2): enum body for {name} */\n"));
                } else if let Some(def) = layout_emit::emit_aggregate(model, names, *ty, name) {
                    out.push_str(&def);
                }
            }
            Node::Slice { elem, const_data } => {
                out.push_str(&types::emit_slice_struct(
                    model,
                    names,
                    name,
                    *elem,
                    *const_data,
                ));
            }
            Node::TraitObj { dynamic } => {
                let vtable = format!("{}_VTable", trait_label(model, *dynamic));
                out.push_str(&types::emit_trait_obj_struct(name, &vtable));
            }
        }
    }
    out
}

/// Bank 4 (section 15): function prototypes for every function instance
/// and `extern` declarations for every static. Task-d hook (empty here).
fn bank4_declarations(model: &RegseqModel) -> String {
    let _ = model;
    // TODO(task d): section 15 bank 4 -- prototypes + extern statics.
    String::new()
}

/// Bank 5 (section 15): function bodies + static initializers, F4 order.
/// Task-d hook (empty here).
fn bank5_definitions(model: &RegseqModel) -> String {
    let _ = model;
    // TODO(task d): section 15 bank 5 -- function bodies + static inits.
    String::new()
}

/// Errors from generic-C emission.
///
/// `#[non_exhaustive]` lets later tasks add variants without a breaking
/// change.
#[derive(Debug)]
#[non_exhaustive]
pub enum EmitError {
    /// The generic-C backend lowering is not yet implemented. Retained as
    /// a placeholder; not returned by the emit path.
    NotImplemented,
    /// A section 15 by-value containment cycle was detected. Sized Rust
    /// types cannot contain themselves by value, so this signals model
    /// corruption rather than a legal input.
    ContainmentCycle,
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmitError::NotImplemented => write!(f, "generic-C emission not implemented"),
            EmitError::ContainmentCycle => {
                write!(
                    f,
                    "generic-C emission: by-value containment cycle (model corruption)"
                )
            }
        }
    }
}

impl std::error::Error for EmitError {}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
