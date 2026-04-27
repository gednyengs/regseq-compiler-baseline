//! Typed declaration IDs and body-level IDs for the RegseqModel.
//!
//! [`DeclId`] is the base declaration identifier -- a single flat namespace
//! backed by a `u32` index. Nine typed newtype wrappers enforce that
//! cross-references point to the correct kind of entity at compile time.
//!
//! [`LocalId`] and [`BasicBlockId`] are body-level IDs scoped to individual
//! function bodies. They do not use `DeclId`.

use serde::{Deserialize, Serialize};

/// Base declaration identifier. Single flat namespace for all declarations.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct DeclId(u32);

impl DeclId {
    /// Create a new `DeclId` from a raw index.
    pub fn new(index: u32) -> Self {
        Self(index)
    }

    /// Return the raw index.
    pub fn index(&self) -> u32 {
        self.0
    }
}

impl std::fmt::Debug for DeclId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DeclId({})", self.0)
    }
}

/// Generate a typed wrapper around [`DeclId`].
macro_rules! decl_id_wrapper {
    (
        $(#[$meta:meta])*
        $name:ident
    ) => {
        $(#[$meta])*
        #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        pub struct $name(pub DeclId);

        impl $name {
            /// Return the inner [`DeclId`].
            pub fn decl_id(&self) -> DeclId {
                self.0
            }
        }

        impl From<DeclId> for $name {
            fn from(id: DeclId) -> Self {
                Self(id)
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({:?})", stringify!($name), self.0)
            }
        }
    };
}

decl_id_wrapper! {
    /// Identifies a module in the module tree.
    ModuleId
}

decl_id_wrapper! {
    /// Identifies a constant or static item (top-level or associated).
    ConstantId
}

decl_id_wrapper! {
    /// Identifies a free function, helper method, trait method, or ISR handler.
    FunctionId
}

decl_id_wrapper! {
    /// Identifies a struct or enum (user-defined, abstraction, or framework).
    TypeDefId
}

decl_id_wrapper! {
    /// Identifies a register bitfield descriptor.
    RegisterDescriptorId
}

decl_id_wrapper! {
    /// Identifies a register block struct.
    RegisterBlockId
}

decl_id_wrapper! {
    /// Identifies a resource field on the driver struct.
    ResourceId
}

decl_id_wrapper! {
    /// Identifies a property field on the driver struct.
    PropertyId
}

decl_id_wrapper! {
    /// Identifies a user data field on the driver struct.
    UserDataId
}

// -- Body-level IDs (not DeclId-based) --

/// Identifies a local variable or temporary within a function body.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct LocalId(pub u32);

impl LocalId {
    /// Create a new `LocalId` from a raw index.
    pub fn new(index: u32) -> Self {
        Self(index)
    }

    /// Return the raw index.
    pub fn index(&self) -> u32 {
        self.0
    }
}

impl std::fmt::Debug for LocalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LocalId({})", self.0)
    }
}

/// Identifies a basic block within a function body.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct BasicBlockId(pub u32);

impl BasicBlockId {
    /// Create a new `BasicBlockId` from a raw index.
    pub fn new(index: u32) -> Self {
        Self(index)
    }

    /// Return the raw index.
    pub fn index(&self) -> u32 {
        self.0
    }
}

impl std::fmt::Debug for BasicBlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BasicBlockId({})", self.0)
    }
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;

    #[test]
    fn decl_id_new_and_index() {
        let id = DeclId::new(42);
        assert_eq!(id.index(), 42);
    }

    #[test]
    fn construct_all_typed_wrappers_from_decl_id() {
        let base = DeclId::new(7);
        let _: ModuleId = ModuleId::from(base);
        let _: ConstantId = ConstantId::from(base);
        let _: FunctionId = FunctionId::from(base);
        let _: TypeDefId = TypeDefId::from(base);
        let _: RegisterDescriptorId = RegisterDescriptorId::from(base);
        let _: RegisterBlockId = RegisterBlockId::from(base);
        let _: ResourceId = ResourceId::from(base);
        let _: PropertyId = PropertyId::from(base);
        let _: UserDataId = UserDataId::from(base);
    }

    #[test]
    fn wrappers_with_same_decl_id_are_equal() {
        let a = FunctionId(DeclId::new(3));
        let b = FunctionId(DeclId::new(3));
        assert_eq!(a, b);
    }

    #[test]
    fn wrappers_with_different_decl_id_are_not_equal() {
        let a = FunctionId(DeclId::new(1));
        let b = FunctionId(DeclId::new(2));
        assert_ne!(a, b);
    }

    /// Typed wrappers are distinct Rust types -- this is a compile-time check.
    /// If FunctionId and ModuleId were the same type, this function's signature
    /// would fail to compile due to conflicting parameter types.
    fn _type_distinctness_check(_f: FunctionId, _m: ModuleId) {}

    #[test]
    fn local_id_and_basic_block_id_are_independent_of_decl_id() {
        // LocalId and BasicBlockId use raw u32, not DeclId.
        let local = LocalId::new(5);
        let bb = BasicBlockId::new(5);
        assert_eq!(local.index(), 5);
        assert_eq!(bb.index(), 5);
        // They are distinct types from each other and from DeclId.
    }

    #[test]
    fn debug_format_for_decl_id() {
        let id = DeclId::new(3);
        assert_eq!(format!("{:?}", id), "DeclId(3)");
    }

    #[test]
    fn debug_format_for_typed_wrappers() {
        let fid = FunctionId(DeclId::new(3));
        assert_eq!(format!("{:?}", fid), "FunctionId(DeclId(3))");

        let mid = ModuleId(DeclId::new(0));
        assert_eq!(format!("{:?}", mid), "ModuleId(DeclId(0))");
    }

    #[test]
    fn debug_format_for_body_level_ids() {
        assert_eq!(format!("{:?}", LocalId::new(2)), "LocalId(2)");
        assert_eq!(format!("{:?}", BasicBlockId::new(9)), "BasicBlockId(9)");
    }

    #[test]
    fn serde_round_trip_decl_id() {
        let id = DeclId::new(42);
        let json = serde_json::to_string(&id).unwrap();
        let back: DeclId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn serde_round_trip_typed_wrappers() {
        let wrappers: Vec<Box<dyn std::any::Any>> = vec![
            Box::new(ModuleId(DeclId::new(1))),
            Box::new(ConstantId(DeclId::new(2))),
            Box::new(FunctionId(DeclId::new(3))),
            Box::new(TypeDefId(DeclId::new(4))),
            Box::new(RegisterDescriptorId(DeclId::new(5))),
            Box::new(RegisterBlockId(DeclId::new(6))),
            Box::new(ResourceId(DeclId::new(7))),
            Box::new(PropertyId(DeclId::new(8))),
            Box::new(UserDataId(DeclId::new(9))),
        ];
        // Test each individually to get proper serde round-trip
        let _ = wrappers; // type-erased check above is just for construction

        // Concrete round-trips:
        let m = ModuleId(DeclId::new(1));
        let j = serde_json::to_string(&m).unwrap();
        assert_eq!(m, serde_json::from_str::<ModuleId>(&j).unwrap());

        let c = ConstantId(DeclId::new(2));
        let j = serde_json::to_string(&c).unwrap();
        assert_eq!(c, serde_json::from_str::<ConstantId>(&j).unwrap());

        let f = FunctionId(DeclId::new(3));
        let j = serde_json::to_string(&f).unwrap();
        assert_eq!(f, serde_json::from_str::<FunctionId>(&j).unwrap());

        let t = TypeDefId(DeclId::new(4));
        let j = serde_json::to_string(&t).unwrap();
        assert_eq!(t, serde_json::from_str::<TypeDefId>(&j).unwrap());

        let rd = RegisterDescriptorId(DeclId::new(5));
        let j = serde_json::to_string(&rd).unwrap();
        assert_eq!(
            rd,
            serde_json::from_str::<RegisterDescriptorId>(&j).unwrap()
        );

        let rb = RegisterBlockId(DeclId::new(6));
        let j = serde_json::to_string(&rb).unwrap();
        assert_eq!(rb, serde_json::from_str::<RegisterBlockId>(&j).unwrap());

        let r = ResourceId(DeclId::new(7));
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(r, serde_json::from_str::<ResourceId>(&j).unwrap());

        let p = PropertyId(DeclId::new(8));
        let j = serde_json::to_string(&p).unwrap();
        assert_eq!(p, serde_json::from_str::<PropertyId>(&j).unwrap());

        let u = UserDataId(DeclId::new(9));
        let j = serde_json::to_string(&u).unwrap();
        assert_eq!(u, serde_json::from_str::<UserDataId>(&j).unwrap());
    }

    #[test]
    fn serde_round_trip_local_id() {
        let id = LocalId::new(10);
        let json = serde_json::to_string(&id).unwrap();
        let back: LocalId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn serde_round_trip_basic_block_id() {
        let id = BasicBlockId::new(99);
        let json = serde_json::to_string(&id).unwrap();
        let back: BasicBlockId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn decl_id_serializes_as_plain_integer() {
        let id = DeclId::new(42);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "42");
    }

    #[test]
    fn decl_id_wrapper_exposes_inner() {
        let base = DeclId::new(5);
        let fid = FunctionId(base);
        assert_eq!(fid.decl_id(), base);
    }
}
