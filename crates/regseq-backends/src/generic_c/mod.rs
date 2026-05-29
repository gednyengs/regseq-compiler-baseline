//! The generic-C backend driver: the `generic-c-backend.md` section 15
//! five-bank single-translation-unit assembly.
//!
//! Task b shipped the model-independent scaffolding (the bank-ordering
//! driver + bank 1, the [`prelude`]). Task c1 fills banks 2-3: the
//! type-system foundation -- the `Ty`->C spelling ([`types`]), the D1 type
//! naming ([`naming`] + [`NameTable`]), the section 14 / D10 field-offset
//! aggregate emitter ([`layout_emit`]), the section 3.2 fat-pointer struct
//! shapes, and the section 15 by-value-containment discovery + topo-sort
//! that orders the definitions. Task d1 fills banks 4-5 for the scalar /
//! pointer / control-flow subset: prototypes + `extern` statics (bank 4),
//! function bodies ([`body_emit`]) + scalar / pointer static initializers
//! (bank 5), and widens the section 15 discovery roots to signature / local
//! / static / const types.
//!
//! Scope (tasks c1 + c2): emits compilable C type definitions for sized,
//! non-`dyn` aggregates (struct / union / tuple / closure / array / slice /
//! enum / primitives), each with `_Static_assert` layout guards. Task c2
//! filled the enum body ([`enum_emit`], the section 13 / D9 union-of-views
//! form); `dyn` vtable construction + virtual dispatch are deferred (only
//! the `trait_obj` shape + a VTable forward-decl ship here). Task d1 adds
//! the section 7 body lowering; structured-data bodies (field / index /
//! downcast access, aggregate rvalues, enum tag-ops) are deferred to d2,
//! each failing loud via [`EmitError::Unsupported`].

use std::collections::{HashMap, HashSet};

use regseq_model::{
    CAbiLowering, CAbiShape, FnAbi, FnSig, FnSigId, InlineHint, InstanceId, InstanceKind,
    InstanceRef, Linkage, Mutability, Provenance, RegseqModel, Ty, TyId,
};

pub mod prelude;

mod body_emit;
mod enum_emit;
mod layout_emit;
mod naming;
mod types;

pub use naming::{entry_c_symbol, EntrySymbol};

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

/// The task-11 instance-symbol table: a pure function of the model mapping
/// every `Function` / `Static` instance to its final, linkage-aware C symbol
/// via [`naming::instance_symbol_base`]. External-linkage symbols (`NoMangle`
/// -- including `export_name` -- and `Foreign`) are `sanitize(emission_name)`
/// with **no** generic suffix (an ABI contract: stable across monos);
/// internal symbols (`Default` / `Weak`) are `sanitize(emission_name) +
/// generic_suffix(args)` with a `_clash_<n>` net, mirroring [`NameTable`].
/// Built once in [`emit_generic_c`] and threaded
/// through banks 4 / 5 + [`body_emit`] so the prototype, the definition,
/// every call / `&fn` / `Reloc`, and every static reference all spell the
/// same symbol. `Const` instances materialise at use sites (no standalone
/// symbol) and aggregate instances are type *tags* (a different C
/// namespace), so neither is in this table.
struct InstanceSymbols {
    symbols: HashMap<InstanceId, String>,
}

impl InstanceSymbols {
    /// The final C symbol for instance `id`. Panics if `id` is not a
    /// `Function` / `Static` instance (the only kinds the table covers).
    fn symbol(&self, id: InstanceId) -> &str {
        self.symbols
            .get(&id)
            .expect("InstanceSymbols queried for an instance without an emitted symbol")
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
/// containment), (4) prototypes + `extern` statics, (5) function bodies +
/// static initializers. The output is a *pure function of the model* (no
/// wall-clock / iteration nondeterminism) so the snapshot oracle's
/// byte-stability holds.
///
/// Returns [`EmitError::ContainmentCycle`] if the section 15 by-value
/// containment graph has a cycle (impossible for sized Rust types, so it
/// signals model corruption), or [`EmitError::Unsupported`] if a body /
/// static-init construct outside the task-d1 scalar / pointer / control-flow
/// subset is reached; otherwise `Ok`.
pub fn emit_generic_c(model: &RegseqModel) -> Result<String, EmitError> {
    let discovered = discover(model);
    let names = build_name_table(model, &discovered);
    let syms = build_instance_symbols(model);
    let topo = topo_sort(model, &discovered)?;
    // Task d2.2 Change F: recover each `ConstMemoryStatic`'s consumer pointee
    // storage type, so banks 4 / 5 declare it as the type the consumer reads
    // (sound + naturally aligned) instead of the bridge's `Array(u8, len)`.
    let cms_storage = const_memory_static_storage(model)?;

    let mut out = String::new();
    out.push_str(prelude::PRELUDE);
    out.push_str(&prelude::helper_block());
    out.push_str(&bank2_type_forward_decls(model, &names, &discovered));
    out.push_str(&bank3_type_definitions(model, &names, &topo));
    out.push_str(&bank4_declarations(model, &names, &syms, &cms_storage)?);
    out.push_str(&bank5_definitions(model, &names, &syms, &cms_storage)?);
    Ok(out)
}

/// The by-value field / array-element edges out of an aggregate `ty` (the
/// section 15 edge set shared by discovery and ordering): edges through
/// `FieldLayout.ty` (struct / union / tuple / enum-payload / closure-capture
/// fields) + the element of a reached `Ty::Array`; a field that is a `Ref`
/// / `Ptr` to an unsized pointee edges to its fat struct (`rust_slice_<T>` /
/// `trait_obj_<Trait>`; `rust_str` is bank 1, always complete). No edge
/// through `Ref` / `Ptr` to a sized pointee, `FnPtr`, or `FnDef`.
fn agg_edges(model: &RegseqModel, ty: TyId) -> Vec<Node> {
    raw_field_tys(model, ty)
        .into_iter()
        .filter(|field_ty| model.size_align_of(*field_ty).size > 0)
        .filter_map(|field_ty| containment_target(model, field_ty))
        .collect()
}

/// Discovery edges out of `node` (used by [`discover`] / [`dfs_discover`]):
/// the nodes that must be *named + bank-2 forward-declared* because `node`
/// mentions them. Broader than [`ordering_edges`]: a `Node::Slice`'s `data`
/// slot is a pointer to its element, so the element must be named +
/// forward-declared (resolve `elem` through any `Ty::Array` to its base
/// aggregate / fat node via [`containment_target`]) even though a pointer to
/// a forward-declared type needs no complete-before ordering. Without this
/// edge an aggregate reachable *only* through a slice never enters the
/// [`NameTable`], so spelling the `data` field panics. `Node::TraitObj` has
/// no discovery edge: its `data` is `void *` and the `<Trait>_VTable` is only
/// forward-declared (deferred).
fn discovery_edges(model: &RegseqModel, node: &Node) -> Vec<Node> {
    match node {
        Node::TraitObj { .. } => Vec::new(),
        Node::Slice { elem, .. } => discovery_target(model, *elem).into_iter().collect(),
        Node::Agg(ty) => raw_field_tys(model, *ty)
            .into_iter()
            .filter(|field_ty| model.size_align_of(*field_ty).size > 0)
            .filter_map(|field_ty| discovery_target(model, field_ty))
            .collect(),
    }
}

/// The discovery-edge target of a type (task d1 Change A2): like
/// [`containment_target`], but **also follows a sized `Ref` / `Ptr` edge to
/// its pointee's discovery node** (recursing). A `&(u8, u32)` parameter /
/// field reaches a structural tuple that the nominal-root safety net
/// (`bridge/mod.rs`) misses, so without this edge spelling its `data` /
/// declarator would panic the [`NameTable`]. This is **naming /
/// forward-decl only** -- the by-value [`ordering_edges`] still stops at
/// pointers, so no false [`EmitError::ContainmentCycle`] is introduced.
fn discovery_target(model: &RegseqModel, ty: TyId) -> Option<Node> {
    match &model.types[ty] {
        Ty::Struct { .. }
        | Ty::Union { .. }
        | Ty::Enum { .. }
        | Ty::Tuple { .. }
        | Ty::Closure(_, _) => Some(Node::Agg(ty)),
        Ty::Array(elem, _) => discovery_target(model, *elem),
        Ty::Ref(pointee, _) | Ty::Ptr(pointee, _) => match unsized_pointee_node(model, ty) {
            // An unsized pointee lowers to a by-value fat struct.
            Some(node) => Some(node),
            // A sized pointee: follow the pointer for naming / forward-decl.
            None => discovery_target(model, *pointee),
        },
        _ => None,
    }
}

/// By-value ordering edges out of `node` (used by [`topo_sort`] /
/// [`topo_visit`]): the nodes that must be *complete before* `node` is
/// emitted. Narrower than [`discovery_edges`]: a `Node::Slice` whose element
/// is a struct / union / tuple / closure spells `data` as `T *`, so a
/// forward-decl suffices (no ordering edge -- this is what keeps the legal
/// self-reference `struct Inner { xs: &[Inner] }` from being a false
/// [`EmitError::ContainmentCycle`]). The single exception: a slice whose
/// element is a `Ty::Array(base, N)` spells `data` as `base (*)[N]`, and a C
/// array type needs its element complete even behind a pointer, so it edges
/// (ordering) to `base`'s aggregate. `Node::TraitObj` has no ordering edge.
fn ordering_edges(model: &RegseqModel, node: &Node) -> Vec<Node> {
    match node {
        Node::TraitObj { .. } => Vec::new(),
        Node::Slice { elem, .. } => slice_array_base_node(model, *elem).into_iter().collect(),
        Node::Agg(ty) => agg_edges(model, *ty),
    }
}

/// The ordering edge for a slice element: `Some` only when `elem` is a
/// `Ty::Array` (recursively through nested arrays) whose base resolves to an
/// aggregate / fat node (which must be complete before the slice's
/// pointer-to-array `data` slot); `None` for a non-array element (a pointer
/// to a forward-declared struct suffices) or an array of scalars (a scalar
/// base is always complete).
fn slice_array_base_node(model: &RegseqModel, elem: TyId) -> Option<Node> {
    match &model.types[elem] {
        Ty::Array(_, _) => containment_target(model, elem),
        _ => None,
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

/// Discover every aggregate / fat-pointer node that banks 2-5 must name +
/// forward-declare. Returns first-seen order, a deterministic function of
/// `InstanceId` order (plus per-instance position order) + the section 15
/// edge set.
///
/// Task d1 (Change A1) widened the roots beyond the nominal aggregates:
/// in addition to every `InstanceKind::Struct` / `Union` / `Enum` layout,
/// `discover` roots on the **effective emitted types** of every function
/// signature position (C-ABI effective for `abi == C`, raw otherwise),
/// every body-local type, and every static / const type, so banks 4 / 5
/// can spell them. Change A2 broadened [`discovery_target`] to follow a
/// sized `Ref` / `Ptr` to its pointee (naming / forward-decl only), so a
/// structural tuple / closure behind a pointer is named too. The
/// **ordering** machinery ([`ordering_edges`] / [`topo_sort`]) is
/// unchanged, so no false [`EmitError::ContainmentCycle`] is introduced and
/// c1 / c2 output is preserved for models without such roots.
fn discover(model: &RegseqModel) -> Vec<Node> {
    let mut seen: HashSet<Node> = HashSet::new();
    let mut order: Vec<Node> = Vec::new();
    let root = |ty: TyId, seen: &mut HashSet<Node>, order: &mut Vec<Node>| {
        if let Some(node) = discovery_target(model, ty) {
            dfs_discover(model, node, seen, order);
        }
    };
    for instance in model.instances.iter() {
        match &instance.kind {
            InstanceKind::Struct { layout }
            | InstanceKind::Enum { layout }
            | InstanceKind::Union { layout } => {
                dfs_discover(model, Node::Agg(*layout), &mut seen, &mut order);
            }
            InstanceKind::Function(data) => {
                // Each emitted signature position (C-ABI effective for
                // `abi == C`), inputs then output, then every body local.
                let fn_sig = model.sigs.resolve(data.fn_sig);
                let lowering = c_abi_lowering(model, data.fn_sig);
                for i in 0..fn_sig.inputs.len() {
                    root(
                        effective_input_ty(fn_sig, lowering, i),
                        &mut seen,
                        &mut order,
                    );
                }
                root(effective_output_ty(fn_sig, lowering), &mut seen, &mut order);
                if let Some(body) = data.body() {
                    for decl in body.local_decls.iter() {
                        root(decl.ty, &mut seen, &mut order);
                    }
                }
            }
            InstanceKind::Static { ty, .. } | InstanceKind::Const { ty, .. } => {
                root(*ty, &mut seen, &mut order);
            }
        }
    }
    order
}

/// Pre-order DFS collecting nodes into first-seen `order`.
fn dfs_discover(model: &RegseqModel, node: Node, seen: &mut HashSet<Node>, order: &mut Vec<Node>) {
    if !seen.insert(node.clone()) {
        return;
    }
    order.push(node.clone());
    for child in discovery_edges(model, &node) {
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
    for child in ordering_edges(model, node) {
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

/// Build the task-11 [`InstanceSymbols`] table (Change B). Construction is a
/// deterministic, two-pass function of the model in `InstanceId` order:
///
/// 1. Reserve into `used` the prelude ordinary-namespace identifiers (the
///    `regseq_*` helpers) **and** every external instance's base name (its
///    final symbol: `sanitize(emission_name)`, no suffix), so a derived
///    internal symbol can never be renamed onto an ABI name or a prelude
///    helper.
/// 2. Assign each `Function` / `Static` instance a symbol: an external
///    instance takes its base as-is (already reserved; an external / external
///    collision is a model / ABI error that surfaces as a duplicate C symbol,
///    never a silent rename); an internal instance takes its base
///    `_clash_<n>`-disambiguated against `used` (the [`build_name_table`]
///    algorithm).
fn build_instance_symbols(model: &RegseqModel) -> InstanceSymbols {
    let mut symbols: HashMap<InstanceId, String> = HashMap::new();
    let mut used: HashSet<String> = prelude_ordinary_idents().into_iter().collect();

    // Pass 1: reserve the external symbols (taken as-is, never renamed).
    for idx in 0..model.instances.len() {
        let id = InstanceId::from_u32(idx as u32);
        let instance = &model.instances[id];
        if emits_ordinary_symbol(&instance.kind) && is_external_linkage(model, instance.item) {
            used.insert(naming::instance_symbol_base(model, instance));
        }
    }

    // Pass 2: assign each function / static a final symbol.
    for idx in 0..model.instances.len() {
        let id = InstanceId::from_u32(idx as u32);
        let instance = &model.instances[id];
        if !emits_ordinary_symbol(&instance.kind) {
            continue;
        }
        let base = naming::instance_symbol_base(model, instance);
        let symbol = if is_external_linkage(model, instance.item) {
            base
        } else {
            let mut candidate = base.clone();
            let mut n = 1usize;
            while used.contains(&candidate) {
                candidate = format!("{base}_clash_{n}");
                n += 1;
            }
            used.insert(candidate.clone());
            candidate
        };
        symbols.insert(id, symbol);
    }

    InstanceSymbols { symbols }
}

/// Whether an instance kind emits an ordinary-namespace C symbol (a bank-4 /
/// bank-5 function or static). `Const` instances materialise at use sites and
/// aggregate instances are type tags (a different C namespace), so neither is
/// in the instance-symbol table.
fn emits_ordinary_symbol(kind: &InstanceKind) -> bool {
    matches!(
        kind,
        InstanceKind::Function(_) | InstanceKind::Static { .. }
    )
}

/// Whether an item's linkage is external (an ABI contract -> a suffix-free
/// symbol): `NoMangle` (which also covers `export_name`) or `Foreign`.
fn is_external_linkage(model: &RegseqModel, item: regseq_model::ItemId) -> bool {
    matches!(
        model.items[item].linkage,
        Linkage::NoMangle | Linkage::Foreign
    )
}

/// The prelude's ordinary-namespace (function) identifiers that an internal
/// derived instance symbol must not shadow (Change B step 1). Mirrors the
/// `regseq_*` helpers emitted by [`prelude::helper_block`] -- the two fixed
/// panic helpers plus the per-type `regseq_{add,sub,mul}_<T>` and
/// `regseq_<fprefix>_to_<T>` families over the prelude's fixed integer C
/// type set. The tag-namespace `struct regseq_overflow_<T>` / `struct
/// rust_str` are a different C namespace and are deliberately not reserved;
/// libc names (`memcpy` / `printf` / `abort`) are an accepted low-risk edge
/// (a `Default` item rendering to exactly one of those fails the C compile
/// loudly, never silently). The
/// `prelude_ordinary_idents_all_defined_in_prelude` test pins this list
/// against the actually-emitted prelude text.
fn prelude_ordinary_idents() -> Vec<String> {
    // The integer C type spellings of the prelude's fixed helper set
    // (prelude.rs `INT_TYPES`) and the float prefixes (`FLOAT_TYPES`).
    const INT_CTYPES: [&str; 12] = [
        "int8_t",
        "int16_t",
        "int32_t",
        "int64_t",
        "intptr_t",
        "__int128",
        "uint8_t",
        "uint16_t",
        "uint32_t",
        "uint64_t",
        "size_t",
        "__uint128_t",
    ];
    const FLOAT_PREFIXES: [&str; 4] = ["f32", "f64", "f16", "f128"];

    let mut idents = vec!["regseq_panic".to_string(), "regseq_unreachable".to_string()];
    for ty in INT_CTYPES {
        for op in ["add", "sub", "mul"] {
            idents.push(format!("regseq_{op}_{ty}"));
        }
    }
    for prefix in FLOAT_PREFIXES {
        for ity in INT_CTYPES {
            idents.push(format!("regseq_{prefix}_to_{ity}"));
        }
    }
    idents
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
/// section 14 emitter; enums go through the section 13 / D9 union-of-views
/// emitter ([`enum_emit`]); fat structs emit their section 3.2 shape.
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
                    // Section 13 / D9: the offset-faithful union-of-views
                    // enum body, reusing the section 14 primitive.
                    out.push_str(&enum_emit::emit_enum(model, names, *ty, name));
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

/// The C-ABI lowering side-table entry for a signature, when present. Hand-
/// built models may intern signatures without a parallel
/// `sig_c_abi_lowerings` slot, so this bounds-checks rather than indexing
/// (an absent entry means "use the raw `FnSig` types").
fn c_abi_lowering(model: &RegseqModel, sig: FnSigId) -> Option<&CAbiLowering> {
    let idx = sig.as_u32() as usize;
    if idx < model.sig_c_abi_lowerings.len() {
        model.sig_c_abi_lowerings[sig].as_ref()
    } else {
        None
    }
}

/// The effective emitted type of input position `i` (section 15): the C-ABI
/// `emit_as` for a `Substituted` position of a `C`-ABI signature, otherwise
/// the position's own `FnSig` type.
fn effective_input_ty(fn_sig: &FnSig, lowering: Option<&CAbiLowering>, i: usize) -> TyId {
    match lowering {
        Some(l) if fn_sig.abi == FnAbi::C => match &l.inputs[i] {
            CAbiShape::Substituted { emit_as, .. } => *emit_as,
            CAbiShape::Direct => fn_sig.inputs[i],
        },
        _ => fn_sig.inputs[i],
    }
}

/// The effective emitted return type (section 15), mirroring
/// [`effective_input_ty`].
fn effective_output_ty(fn_sig: &FnSig, lowering: Option<&CAbiLowering>) -> TyId {
    match lowering {
        Some(l) if fn_sig.abi == FnAbi::C => match &l.output {
            CAbiShape::Substituted { emit_as, .. } => *emit_as,
            CAbiShape::Direct => fn_sig.output,
        },
        _ => fn_sig.output,
    }
}

/// Whether `ty` is zero-sized (so it is omitted as a function parameter /
/// return slot). A bare unsized marker is never a sized signature position,
/// so the `size_align_of` guard only needs the aggregate / scalar path.
fn is_zst(model: &RegseqModel, ty: TyId) -> bool {
    match &model.types[ty] {
        Ty::Str | Ty::Slice(_) | Ty::Dynamic { .. } => false,
        _ => model.size_align_of(ty).size > 0,
    }
    .then_some(())
    .is_none()
}

/// The section 17 inline / weak / link-section qualifier prefix for a
/// function instance (everything but `extern` and `_Noreturn`). Shared by
/// the bank-4 prototype and the bank-5 definition so the two stay
/// consistent.
fn fn_qualifiers(model: &RegseqModel, item: regseq_model::ItemId) -> String {
    let item = &model.items[item];
    let mut q = String::new();
    match item.inline_hint {
        InlineHint::Hint => q.push_str("inline "),
        InlineHint::Always => q.push_str("inline __attribute__((always_inline)) "),
        InlineHint::Never => q.push_str("__attribute__((noinline)) "),
        InlineHint::None => {}
    }
    if item.linkage == Linkage::Weak {
        q.push_str("__attribute__((weak)) ");
    }
    if let Some(section) = item.link_section {
        q.push_str(&format!(
            "__attribute__((section(\"{}\"))) ",
            model.strings.resolve(section)
        ));
    }
    q
}

/// The section 17 / section 15 function signature `<quals> <ret>
/// <name>(<params>)` (no trailing `;` or body). `named` controls whether
/// the parameters carry their body-local names (bank 5 definition) or are
/// bare types (bank 4 prototype). Parameters / return are spelled from the
/// C-ABI effective types; ZST positions are omitted. `c_variadic` appends a
/// trailing `, ...`; a `Never` return is `_Noreturn`.
fn fn_signature(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    id: InstanceId,
    instance: &regseq_model::Instance,
    named: bool,
) -> String {
    let data = instance
        .kind
        .as_function()
        .expect("fn_signature on a non-function instance");
    let fn_sig = model.sigs.resolve(data.fn_sig);
    let lowering = c_abi_lowering(model, data.fn_sig);
    let emission = emission_ident(syms, id);

    let mut params: Vec<String> = Vec::new();
    for i in 0..fn_sig.inputs.len() {
        let eff = effective_input_ty(fn_sig, lowering, i);
        if is_zst(model, eff) {
            continue;
        }
        let pname = if named {
            // Argument locals are `local_decls[1..=arg_count]`.
            data.body()
                .map(|body| {
                    body_emit::local_c_name(
                        model,
                        body,
                        regseq_model::LocalId::from_u32(i as u32 + 1),
                    )
                })
                .unwrap_or_default()
        } else {
            String::new()
        };
        params.push(types::declare(model, names, eff, &pname));
    }

    let mut params_str = if params.is_empty() {
        "void".to_string()
    } else {
        params.join(", ")
    };
    if fn_sig.c_variadic {
        params_str.push_str(", ...");
    }

    let out_ty = effective_output_ty(fn_sig, lowering);
    let ret = if is_zst(model, out_ty) {
        "void".to_string()
    } else {
        types::c_type(model, names, out_ty)
    };

    let quals = fn_qualifiers(model, instance.item);
    let noreturn = if matches!(model.types[fn_sig.output], Ty::Never) {
        "_Noreturn "
    } else {
        ""
    };
    format!("{quals}{noreturn}{ret} {emission}({params_str})")
}

/// The final C symbol for instance `id`, looked up in the task-11
/// [`InstanceSymbols`] table. The bridge mints `emission_name` as the
/// generic-agnostic, possibly-not-C-valid `DefPath.rendered`, so the backend
/// *derives* the final symbol (`sanitize(rendered)` with no suffix for
/// external linkage, `sanitize + generic_suffix` clash-netted for internal)
/// -- see [`build_instance_symbols`].
/// Routing this single helper (used by the bank-4 / bank-5 function
/// prototype + definition and the static decl + def) through the table keeps
/// every instance-symbol site in agreement.
fn emission_ident(syms: &InstanceSymbols, id: InstanceId) -> &str {
    syms.symbol(id)
}

/// Whether an instance's source item carries a diag-log recognition tag
/// (section 15 diag-log exclusion: skipped in banks 4 / 5).
fn is_diag_log_instance(model: &RegseqModel, item: regseq_model::ItemId) -> bool {
    use regseq_model::{AttrData, IntrinsicKind};
    let attrs = &model.domain_attrs.item_attrs;
    if item.as_u32() as usize >= attrs.len() {
        return false;
    }
    attrs[item]
        .iter()
        .any(|a| matches!(a, AttrData::Intrinsic(IntrinsicKind::Log(_))))
}

/// A backend-local descriptor of a `ConstMemoryStatic`'s consumer pointee
/// storage type (d2.2 Change F). The bridge synthesises every
/// `ConstMemoryStatic` as a `Static { ty: Array(u8, byte_len) }`
/// (`const_.rs:221-230`); reading it through `(const T *)&X` for a non-char
/// `T` is strict-aliasing UB and may be mis-aligned. This descriptor records
/// the type the consumer actually reads, so banks 4 / 5 declare `&X` at the
/// right type (same-type cast, naturally aligned -- sound). It references only
/// already-interned element / pointee `TyId`s; it never interns an
/// `Array(elem, meta)` (the bridge interns `Slice(elem)` but not the array
/// wrapper), spelling the array case via the declarator-aware
/// [`types::declare`] instead.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StorageTy {
    /// A sized pointee `T` (thin `&T` / `*const T`): declared `T <ident>`.
    Sized(TyId),
    /// A slice / str backing `[elem; count]`: declared via the
    /// declarator-aware `types::declare(elem, "<ident>[count]")`.
    Array {
        /// The element type (already interned: `u8` / a `Slice(elem)`'s elem).
        elem: TyId,
        /// The element count (the reference metadata / `byte_len / size_of`).
        count: u64,
    },
}

/// The C declaration of a `ConstMemoryStatic` re-typed to `storage`
/// (d2.2 Change F): a sized pointee via [`types::declare`]; an element
/// array via the **declarator-aware** `types::declare(elem, "<ident>[count]")`
/// (not a bare `<c_type(elem)> X[count]` string, which is invalid C for an
/// array-of-array / fn-pointer element).
fn declare_storage(
    model: &RegseqModel,
    names: &NameTable,
    storage: &StorageTy,
    ident: &str,
) -> String {
    match storage {
        StorageTy::Sized(ty) => types::declare(model, names, *ty, ident),
        StorageTy::Array { elem, count } => {
            types::declare(model, names, *elem, &format!("{ident}[{count}]"))
        }
    }
}

/// Task d2.2 Change F pre-pass: recover each `ConstMemoryStatic`'s consumer
/// pointee storage type, returning a deterministic `InstanceId -> StorageTy`
/// map (only `Array(u8, _)`-typed statics -- the CMS shape -- are recorded; a
/// regular static already carries its real type). Scans every const reachable
/// from instances (static inits, const values, function-body operand
/// constants), threading the consumer type the way `const_expr` does. A
/// content-dedup collision (one CMS referenced at two incompatible pointee
/// types, `const_.rs:145-152`) fails the whole emit loud (a sound fail-loud;
/// the clean fix is a bridge type-aware dedup key -- task d2.3-bridge-dedup).
/// No model interning, no new aggregate type, no discovery / topo edge.
fn const_memory_static_storage(
    model: &RegseqModel,
) -> Result<HashMap<InstanceId, StorageTy>, EmitError> {
    let mut map: HashMap<InstanceId, StorageTy> = HashMap::new();
    for idx in 0..model.instances.len() {
        let id = InstanceId::from_u32(idx as u32);
        match &model.instances[id].kind {
            InstanceKind::Static {
                ty,
                init: Some(init),
            } => scan_const(model, model.consts.resolve(*init), *ty, &mut map)?,
            InstanceKind::Const { ty, value } => {
                scan_const(model, model.consts.resolve(*value), *ty, &mut map)?
            }
            InstanceKind::Function(data) => {
                if let Some(body) = data.body() {
                    scan_body(model, body, &mut map)?;
                }
            }
            _ => {}
        }
    }
    Ok(map)
}

/// Scan a const at consumer type `consumer_ty`, recording any
/// `ConstMemoryStatic` reference's pointee storage.
fn scan_const(
    model: &RegseqModel,
    c: &regseq_model::Const,
    consumer_ty: TyId,
    map: &mut HashMap<InstanceId, StorageTy>,
) -> Result<(), EmitError> {
    use regseq_model::Const;
    match c {
        Const::Aggregate(children) => match &model.types[consumer_ty] {
            Ty::Ref(pointee, _) => {
                // The `[Reloc, Uint(meta)]` reference carrier.
                if children.len() == 2 {
                    if let (Const::Reloc(target), Const::Uint(_, meta)) = (
                        model.consts.resolve(children[0]),
                        model.consts.resolve(children[1]),
                    ) {
                        record_ref_storage(model, *target, *pointee, *meta as u64, map)?;
                    }
                }
            }
            _ => scan_aggregate_children(model, children, consumer_ty, map)?,
        },
        Const::Reloc(target) => {
            // A bare reloc consumed by a raw pointer `*const T` / `*mut T`.
            if let Ty::Ptr(pointee, _) = &model.types[consumer_ty] {
                record_sized(model, *target, *pointee, map)?;
            }
        }
        Const::Bytes(bytes) => scan_bytes(model, bytes, consumer_ty, map)?,
        _ => {}
    }
    Ok(())
}

/// Pair a typed-valtree aggregate's children with their field / element types
/// and scan each (so a nested reference carrier / reloc is recovered). Covers
/// every aggregate shape `typed_valtree_aggregate` lowers (Struct / Tuple /
/// Closure via [`layout_emit::field_c_name_ty`]; Array by element), so the
/// scan cannot miss a consumer the emission would re-type.
fn scan_aggregate_children(
    model: &RegseqModel,
    children: &[regseq_model::ConstId],
    consumer_ty: TyId,
    map: &mut HashMap<InstanceId, StorageTy>,
) -> Result<(), EmitError> {
    match &model.types[consumer_ty] {
        Ty::Tuple { .. } | Ty::Struct { .. } | Ty::Closure(_, _) => {
            for (i, child) in children.iter().enumerate() {
                if let Some((_, fty)) = layout_emit::field_c_name_ty(model, consumer_ty, i) {
                    scan_const(model, model.consts.resolve(*child), fty, map)?;
                }
            }
        }
        Ty::Array(elem, _) => {
            let elem = *elem;
            for child in children {
                scan_const(model, model.consts.resolve(*child), elem, map)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Scan a `Const::Bytes` image for nested pointer-field relocations to a
/// `ConstMemoryStatic` (the nested-byte-reloc consumer), threading the field /
/// element type the way [`body_emit`]'s walker does. Covers every shape the
/// byte-image walker reconstructs into a pointer cast: Ref / Ptr, Struct /
/// Tuple / Closure, Array, and an **inline** Enum's payload-variant fields
/// (a multi-view enum byte image with a reloc defers to d2.3, so it emits no
/// CMS cast and needs no scan).
fn scan_bytes(
    model: &RegseqModel,
    bytes: &[regseq_model::Byte],
    consumer_ty: TyId,
    map: &mut HashMap<InstanceId, StorageTy>,
) -> Result<(), EmitError> {
    use regseq_model::Byte;
    match &model.types[consumer_ty] {
        Ty::Ref(pointee, _) | Ty::Ptr(pointee, _) => {
            let size = model.size_align_of(consumer_ty).size as usize;
            if size != 0 && size <= bytes.len() {
                if let Byte::Reloc(r) = bytes[0] {
                    if bytes[..size].iter().all(|b| *b == Byte::Reloc(r)) {
                        record_sized(model, r, *pointee, map)?;
                    }
                }
            }
        }
        Ty::Tuple { .. } | Ty::Struct { .. } | Ty::Closure(_, _) => {
            if let Some(fields) = agg_field_layouts(model, consumer_ty) {
                for f in fields {
                    scan_byte_field(model, bytes, f.ty, f.offset, map)?;
                }
            }
        }
        Ty::Array(elem, n) => {
            let elem = *elem;
            let esize = model.size_align_of(elem).size as usize;
            for i in 0..*n as usize {
                scan_byte_field(model, bytes, elem, (i * esize) as u64, map)?;
            }
        }
        Ty::Enum { .. } => {
            if let Some(shape) = enum_emit::enum_shape(model, consumer_ty) {
                if enum_emit::enum_is_inline(model, shape) {
                    if let Some(variant) = inline_payload_variant_layout(shape) {
                        for f in &variant.fields {
                            scan_byte_field(model, bytes, f.ty, f.offset, map)?;
                        }
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Scan the `[offset .. offset + size_of(field_ty)]` slice of a byte image at
/// `field_ty` (bounds-checked), recursing into [`scan_bytes`].
fn scan_byte_field(
    model: &RegseqModel,
    bytes: &[regseq_model::Byte],
    field_ty: TyId,
    offset: u64,
    map: &mut HashMap<InstanceId, StorageTy>,
) -> Result<(), EmitError> {
    let fsize = model.size_align_of(field_ty).size as usize;
    let off = offset as usize;
    if off + fsize <= bytes.len() {
        scan_bytes(model, &bytes[off..off + fsize], field_ty, map)?;
    }
    Ok(())
}

/// The source / MIR-order `FieldLayout`s of a `Tuple` / `Struct` / `Closure`
/// (a closure resolves through its parallel capture `Ty::Struct`), mirroring
/// [`body_emit`]'s `agg_field_layouts`. `None` for any other type.
fn agg_field_layouts(model: &RegseqModel, ty: TyId) -> Option<&[regseq_model::FieldLayout]> {
    use regseq_model::LayoutKind;
    let layout = match &model.types[ty] {
        Ty::Struct { layout, .. } | Ty::Tuple { layout, .. } => layout,
        Ty::Closure(item, args) => {
            model
                .types
                .iter()
                .find_map(|(_, candidate)| match candidate {
                    Ty::Struct {
                        item: s_item,
                        args: s_args,
                        layout,
                    } if s_item == item && s_args == args => Some(layout),
                    _ => None,
                })?
        }
        _ => return None,
    };
    match &layout.kind {
        LayoutKind::Struct { fields } => Some(fields),
        _ => None,
    }
}

/// The inline payload-bearing variant of an enum (a `Single`'s only variant or
/// a niche untagged variant), whose inline fields a byte image reconstructs.
/// `None` for a fieldless tag-only / multi-view shape.
fn inline_payload_variant_layout(
    shape: &regseq_model::EnumShape,
) -> Option<&regseq_model::VariantLayout> {
    use regseq_model::{EnumShape, TagEncoding};
    match shape {
        EnumShape::Single { variant, .. } => Some(variant),
        EnumShape::Multiple {
            encoding: TagEncoding::Niche {
                untagged_variant, ..
            },
            variants,
            ..
        } => variants.get(untagged_variant.index()),
        _ => None,
    }
}

/// Scan a function body's operand constants (Change F), threading the same
/// consumer type each operand site lowers against in [`body_emit`]: an
/// `Assign(place, rvalue)` (`Use` / `Repeat` at the place / element type; an
/// `Aggregate` at its field / element / variant-field types) plus `Call`
/// argument constants at the callee's parameter types. These cover every
/// position where the bridge surfaces a reference / raw-pointer const to a
/// `ConstMemoryStatic`.
fn scan_body(
    model: &RegseqModel,
    body: &regseq_model::Body,
    map: &mut HashMap<InstanceId, StorageTy>,
) -> Result<(), EmitError> {
    use regseq_model::{StatementKind, TerminatorKind};
    for block in body.basic_blocks.iter() {
        for stmt in block.statements.iter() {
            if let StatementKind::Assign(place, rvalue) = &stmt.kind {
                scan_rvalue(model, body, place, rvalue, map)?;
            }
        }
        if let TerminatorKind::Call { callee, args, .. } = &block.terminator.kind {
            let param_tys = callee_param_tys(model, body, callee);
            for (i, arg) in args.iter().enumerate() {
                if let Some(ty) = param_tys.get(i) {
                    scan_operand_const(model, arg, *ty, map)?;
                }
            }
        }
    }
    Ok(())
}

/// The callee's parameter types (the per-argument consumer types): a `Direct`
/// instance's `FnSig.inputs`, or an `Indirect` callee's `FnPtr` signature
/// inputs (recovered from a bare-local / trailing-`Field` callee operand's
/// type). Empty when the signature is not recoverable. This is the **single
/// source of truth** shared by [`body_emit`]'s call-argument emission and the
/// Change-F pre-pass scan, so the two cannot disagree on which arguments carry
/// a known consumer type (a more deeply projected `FnPtr` callee is threaded
/// in neither -- a non-scalar const argument there fails loud, not mis-typed).
pub(super) fn callee_param_tys(
    model: &RegseqModel,
    body: &regseq_model::Body,
    callee: &regseq_model::CallCallee,
) -> Vec<TyId> {
    use regseq_model::{CallCallee, Operand};
    match callee {
        CallCallee::Direct(id) => model.instances[*id]
            .kind
            .as_function()
            .map(|d| model.sigs.resolve(d.fn_sig).inputs.clone())
            .unwrap_or_default(),
        CallCallee::Indirect(op) => {
            let ty = match op {
                Operand::Copy(p) | Operand::Move(p) => body_emit::place_ty(model, body, p).ok(),
                Operand::Constant(_) => None,
            };
            ty.and_then(|t| match &model.types[t] {
                Ty::FnPtr(sig) => Some(model.sigs.resolve(*sig).inputs.clone()),
                _ => None,
            })
            .unwrap_or_default()
        }
    }
}

/// Scan an assignment's rvalue operand constants at the type each lowers
/// against: `Use` at the destination place type, `Repeat` at the array
/// element type, and an `Aggregate` at its per-operand field / element /
/// variant-field types (the same consumer types [`body_emit`] threads).
fn scan_rvalue(
    model: &RegseqModel,
    body: &regseq_model::Body,
    place: &regseq_model::Place,
    rvalue: &regseq_model::Rvalue,
    map: &mut HashMap<InstanceId, StorageTy>,
) -> Result<(), EmitError> {
    use regseq_model::Rvalue;
    match rvalue {
        Rvalue::Use(op) => {
            // The destination's section 19.4 narrowed type, via the SAME walk
            // the body emitter lowers the const against (`body_emit::place_ty`).
            // A non-`Field` projection (`Index` / `ConstantIndex` / `Subslice`
            // / `Deref`) must be narrowed too: otherwise a CMS-backed reference
            // const assigned through it is missed by the scan, stays the
            // bridge `Array(u8, N)`, and is read through a mismatched-type
            // pointer (the d2.2-soundness bug the `place_consumer_ty` shortcut
            // caused).
            if let Ok(ty) = body_emit::place_ty(model, body, place) {
                scan_operand_const(model, op, ty, map)?;
            }
        }
        Rvalue::Repeat(op, _) => {
            if let Ok(ty) = body_emit::place_ty(model, body, place) {
                if let Ty::Array(elem, _) = model.types[ty] {
                    scan_operand_const(model, op, elem, map)?;
                }
            }
        }
        Rvalue::Aggregate(kind, ops) => scan_aggregate_operands(model, kind, ops, map)?,
        _ => {}
    }
    Ok(())
}

/// Scan an `Aggregate` rvalue's operand constants at their field / element /
/// variant-field consumer types (the same names [`body_emit`] designates).
fn scan_aggregate_operands(
    model: &RegseqModel,
    kind: &regseq_model::AggregateKind,
    ops: &[regseq_model::Operand],
    map: &mut HashMap<InstanceId, StorageTy>,
) -> Result<(), EmitError> {
    use regseq_model::AggregateKind;
    match kind {
        AggregateKind::Struct(ty) | AggregateKind::Tuple(ty) | AggregateKind::Closure(ty) => {
            for (i, op) in ops.iter().enumerate() {
                if let Some((_, fty)) = layout_emit::field_c_name_ty(model, *ty, i) {
                    scan_operand_const(model, op, fty, map)?;
                }
            }
        }
        AggregateKind::Union(ty, field) => {
            if let (Some((_, fty)), Some(op)) = (
                layout_emit::field_c_name_ty(model, *ty, field.index()),
                ops.first(),
            ) {
                scan_operand_const(model, op, fty, map)?;
            }
        }
        AggregateKind::Array(elem) => {
            for op in ops {
                scan_operand_const(model, op, *elem, map)?;
            }
        }
        AggregateKind::Enum(ty, variant) => {
            for (i, op) in ops.iter().enumerate() {
                if let Some((_, fty)) =
                    enum_emit::enum_variant_field(model, *ty, variant.index(), i)
                {
                    scan_operand_const(model, op, fty, map)?;
                }
            }
        }
        AggregateKind::RawPtr(_, _) => {}
    }
    Ok(())
}

/// Scan a single operand: if it is a constant, scan it at `consumer_ty`.
fn scan_operand_const(
    model: &RegseqModel,
    op: &regseq_model::Operand,
    consumer_ty: TyId,
    map: &mut HashMap<InstanceId, StorageTy>,
) -> Result<(), EmitError> {
    if let regseq_model::Operand::Constant(c) = op {
        scan_const(model, model.consts.resolve(*c), consumer_ty, map)?;
    }
    Ok(())
}

/// Record a reference carrier's pointee storage for a `ConstMemoryStatic`
/// `target`: a sized pointee -> `Sized`; `Ty::Slice(elem)` -> `Array{elem,
/// count}`; `Ty::Str` -> `Array{u8, count}` (the u8 element is read from the
/// CMS's own bridge `Array(u8, _)` type, so &str / &[u8] resolve to the same
/// `uint8_t[N]` and participate in the incompatible-pointee collision check);
/// `Ty::Dynamic` is the vtable task (the body emit fails loud).
fn record_ref_storage(
    model: &RegseqModel,
    target: InstanceRef,
    pointee: TyId,
    meta: u64,
    map: &mut HashMap<InstanceId, StorageTy>,
) -> Result<(), EmitError> {
    match &model.types[pointee] {
        Ty::Dynamic { .. } => Ok(()),
        Ty::Str => match cms_array_elem(model, target) {
            Some(elem) => record(model, target, StorageTy::Array { elem, count: meta }, map),
            None => Ok(()),
        },
        Ty::Slice(elem) => record(
            model,
            target,
            StorageTy::Array {
                elem: *elem,
                count: meta,
            },
            map,
        ),
        _ => record(model, target, StorageTy::Sized(pointee), map),
    }
}

/// The element type of a `ConstMemoryStatic` target's bridge `Array(elem, _)`
/// type (always `u8`), for re-typing a `&str` carrier to `Array{u8, count}`.
/// `None` if `target` does not resolve to an `Array`-typed static.
fn cms_array_elem(model: &RegseqModel, target: InstanceRef) -> Option<TyId> {
    let id = resolve_instance(model, target)?;
    match &model.instances[id].kind {
        InstanceKind::Static { ty, .. } => match model.types[*ty] {
            Ty::Array(elem, _) => Some(elem),
            _ => None,
        },
        _ => None,
    }
}

/// Record a sized-pointee storage for a `ConstMemoryStatic` `target` (a thin
/// `&T` / `*const T` or a nested byte-reloc). A non-sized pointee (`Str` /
/// `Slice` / `Dynamic`) is not a thin pointer and is handled elsewhere.
fn record_sized(
    model: &RegseqModel,
    target: InstanceRef,
    pointee: TyId,
    map: &mut HashMap<InstanceId, StorageTy>,
) -> Result<(), EmitError> {
    if matches!(
        model.types[pointee],
        Ty::Str | Ty::Slice(_) | Ty::Dynamic { .. }
    ) {
        return Ok(());
    }
    record(model, target, StorageTy::Sized(pointee), map)
}

/// Record `storage` for `target`, but only for a `ConstMemoryStatic`-shaped
/// `Static` (declared `Array(u8, _)` by the bridge). A conflicting prior
/// record (the content-dedup collision) fails the whole emit loud (a sound
/// fail-loud; the clean fix is a bridge type-aware dedup key -- task
/// d2.3-bridge-dedup). Re-typing an `Array(u8, _)` static is idempotent when
/// the consumer matches its real type, so a user `[u8; N]` static is
/// unaffected.
fn record(
    model: &RegseqModel,
    target: InstanceRef,
    storage: StorageTy,
    map: &mut HashMap<InstanceId, StorageTy>,
) -> Result<(), EmitError> {
    let Some(id) = resolve_instance(model, target) else {
        return Ok(());
    };
    let is_cms_shape = matches!(
        &model.instances[id].kind,
        InstanceKind::Static { ty, .. } if matches!(model.types[*ty], Ty::Array(elem, _) if matches!(model.types[elem], Ty::Uint(regseq_model::UintKind::U8)))
    );
    if !is_cms_shape {
        return Ok(());
    }
    match map.get(&id) {
        Some(existing) if *existing != storage => Err(EmitError::Unsupported(
            "ConstMemoryStatic deduped at incompatible pointee types (bridge content-hash dedup; sound fail-loud - see task d2.3-bridge-dedup)",
        )),
        _ => {
            map.insert(id, storage);
            Ok(())
        }
    }
}

/// The `InstanceId` whose canonical `InstanceRef` equals `target` (mirrors
/// the body emitter's `resolve_reloc`).
fn resolve_instance(model: &RegseqModel, target: InstanceRef) -> Option<InstanceId> {
    (0..model.instances.len()).find_map(|idx| {
        let id = InstanceId::from_u32(idx as u32);
        (model.instance_ref_of(id) == target).then_some(id)
    })
}

/// Bank 4 (section 15): a section 17-qualified prototype for every
/// non-diag-log function instance (`Foreign` -> `extern` decl only) and an
/// `extern [const] T X;` declaration for every static.
fn bank4_declarations(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    cms_storage: &HashMap<InstanceId, StorageTy>,
) -> Result<String, EmitError> {
    let mut out = String::new();
    let mut wrote_header = false;
    let header = |out: &mut String, wrote: &mut bool| {
        if !*wrote {
            out.push_str(
                "\n/* ---------------------------------------------------------------------------\n\
                 \x20* Bank 4 (section 15): prototypes + extern statics.\n\
                 \x20* --------------------------------------------------------------------------- */\n",
            );
            *wrote = true;
        }
    };

    for idx in 0..model.instances.len() {
        let id = InstanceId::from_u32(idx as u32);
        let instance = &model.instances[id];
        match &instance.kind {
            InstanceKind::Function(_) => {
                if is_diag_log_instance(model, instance.item) {
                    continue;
                }
                header(&mut out, &mut wrote_header);
                let foreign = model.items[instance.item].linkage == Linkage::Foreign;
                let prefix = if foreign { "extern " } else { "" };
                out.push_str(&format!(
                    "{prefix}{};\n",
                    fn_signature(model, names, syms, id, instance, false)
                ));
            }
            InstanceKind::Static { ty, .. } => {
                header(&mut out, &mut wrote_header);
                let emission = emission_ident(syms, id);
                let konst = if static_is_const(model, instance.item) {
                    "const "
                } else {
                    ""
                };
                // Task d2.2 Change F: a `ConstMemoryStatic` is declared from
                // its recovered consumer pointee storage type, not the
                // bridge's `Array(u8, len)`.
                let decl = match cms_storage.get(&id) {
                    Some(storage) => declare_storage(model, names, storage, emission),
                    None => types::declare(model, names, *ty, emission),
                };
                out.push_str(&format!("extern {konst}{decl};\n"));
            }
            // Const items materialise at use sites (an address-taken const is
            // a synthetic ConstMemoryStatic, handled by the Static arm);
            // aggregates are emitted in banks 2-3.
            _ => {}
        }
    }
    Ok(out)
}

/// Whether a static's source item declares `static` (`const`) vs
/// `static mut` (non-const), per section 17.
fn static_is_const(model: &RegseqModel, item: regseq_model::ItemId) -> bool {
    use regseq_model::ItemKind;
    matches!(
        model.items[item].kind,
        ItemKind::Static {
            mutability: Mutability::Not
        }
    )
}

/// Bank 5 (section 15): a section 17-qualified definition + section 7 body
/// for every non-foreign, non-diag-log function, and a `[const] T X =
/// <init>;` definition for every `init: Some` static, in F4 order.
fn bank5_definitions(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    cms_storage: &HashMap<InstanceId, StorageTy>,
) -> Result<String, EmitError> {
    let mut out = String::new();
    let mut wrote_header = false;
    let header = |out: &mut String, wrote: &mut bool| {
        if !*wrote {
            out.push_str(
                "\n/* ---------------------------------------------------------------------------\n\
                 \x20* Bank 5 (section 15): function bodies + static initializers.\n\
                 \x20* --------------------------------------------------------------------------- */\n",
            );
            *wrote = true;
        }
    };

    for idx in 0..model.instances.len() {
        let id = InstanceId::from_u32(idx as u32);
        let instance = &model.instances[id];
        match &instance.kind {
            InstanceKind::Function(data) => {
                let foreign = model.items[instance.item].linkage == Linkage::Foreign;
                if foreign || is_diag_log_instance(model, instance.item) {
                    continue;
                }
                let Some(body) = data.body() else {
                    continue;
                };
                header(&mut out, &mut wrote_header);
                let sig = fn_signature(model, names, syms, id, instance, true);
                let block = body_emit::emit_body(model, names, syms, instance, body)?;
                out.push_str(&format!("{sig}{block}"));
            }
            InstanceKind::Static {
                ty,
                init: Some(init),
            } => {
                header(&mut out, &mut wrote_header);
                let emission = emission_ident(syms, id);
                let konst = if static_is_const(model, instance.item) {
                    "const "
                } else {
                    ""
                };
                let init_const = model.consts.resolve(*init);
                // Task d2.2 Change F: a `ConstMemoryStatic` is declared and its
                // byte image reconstructed at its recovered storage type, so
                // `&X` is the consumer pointee type (same-type, aligned).
                let (decl, value) = match cms_storage.get(&id) {
                    Some(storage) => (
                        declare_storage(model, names, storage, emission),
                        body_emit::const_memory_static_initializer(
                            model, names, syms, init_const, storage,
                        )?,
                    ),
                    None => (
                        types::declare(model, names, *ty, emission),
                        body_emit::static_initializer(model, names, syms, init_const, *ty)?,
                    ),
                };
                out.push_str(&format!("{konst}{decl} = {value};\n"));
            }
            _ => {}
        }
    }
    Ok(out)
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
    /// types cannot contain themselves by value, so this normally signals
    /// model corruption. It is also the clean surface for the one shape this
    /// backend cannot spell in C: a genuinely cyclic array-element slice
    /// (e.g. `struct Inner { xs: &[[Inner; 4]] }`), where `Inner` needs the
    /// slice complete as an array element while the slice needs `Inner`
    /// complete by value -- not C-representable, so it errors here rather
    /// than panicking or mis-emitting.
    ContainmentCycle,
    /// A body / static-init construct outside the Phase-1 task-d1 scalar /
    /// pointer / control-flow subset was reached. The `&'static str` names
    /// the deferred construct (e.g. `"Field place projection (task d2)"`).
    /// Task d2 replaces each such site with the real lowering; until then
    /// the backend fails loud rather than mis-emitting.
    Unsupported(&'static str),
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
            EmitError::Unsupported(what) => {
                write!(
                    f,
                    "generic-C emission: unsupported construct: {what} (task d2)"
                )
            }
        }
    }
}

impl std::error::Error for EmitError {}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
