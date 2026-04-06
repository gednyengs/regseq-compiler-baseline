//! Tests for the Layer 2 item hierarchy schema and canonical cross-link model.
//!
//! These tests verify the ownership, identity, and cross-link invariants
//! described in the semantic-IR plan without introducing PL011-specific or
//! OS-specific assumptions.

use regseq_model::declaration::Provenance;
use regseq_model::item::{
    CanonicalPath, ConstValue, ConstantDef, CrossLinkTable, DeviceCallbackRole, DeviceClassKind,
    EnumDef, EnumVariantDef, FieldAttribute, FunctionDef, FunctionKind, FunctionRole, ImplDef,
    InterruptClassDef, Item, ItemAttribute, ItemId, ItemStore, LifecycleRole, Module,
    PrimitiveType, StructDef, StructFieldDef, TraitRef, TypeRef,
};
use regseq_model::semantic::SemanticBody;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_provenance(family: &str) -> Provenance {
    Provenance::new(family)
}

// ---------------------------------------------------------------------------
// Test 1: ItemStore is the canonical owner of item payloads
// ---------------------------------------------------------------------------

#[test]
fn item_store_is_canonical_owner() {
    let mut store = ItemStore::new();
    assert!(store.is_empty());

    let struct_def = StructDef {
        id: ItemId(0), // will be overwritten by store
        canonical_path: CanonicalPath::new("pkg::driver::MyDriver"),
        name: "MyDriver".into(),
        fields: vec![],
        attributes: vec![ItemAttribute::Driver],
        provenance: test_provenance("identity"),
    };

    let id = store.push(Item::Struct(struct_def.clone()));
    assert_eq!(id, ItemId(0));
    assert_eq!(store.len(), 1);
    assert!(!store.is_empty());

    let retrieved = store.get(id).expect("item should exist");
    match retrieved {
        Item::Struct(s) => {
            assert_eq!(s.id, id, "embedded id must match store-assigned id");
            assert_eq!(s.name, "MyDriver");
            assert_eq!(s.canonical_path.as_str(), "pkg::driver::MyDriver");
        }
        _ => panic!("expected Struct item"),
    }

    // Out-of-bounds returns None.
    assert!(store.get(ItemId(999)).is_none());
}

// ---------------------------------------------------------------------------
// Test 1b: ItemStore synchronizes embedded id on push
// ---------------------------------------------------------------------------

#[test]
fn item_store_synchronizes_embedded_id() {
    let mut store = ItemStore::new();

    // Push items with deliberately wrong initial ids.
    let id0 = store.push(Item::Struct(StructDef {
        id: ItemId(999), // wrong -- store must overwrite to 0
        canonical_path: CanonicalPath::new("pkg::A"),
        name: "A".into(),
        fields: vec![],
        attributes: vec![],
        provenance: test_provenance("identity"),
    }));
    assert_eq!(id0, ItemId(0));

    let id1 = store.push(Item::Function(FunctionDef {
        id: ItemId(42), // wrong -- store must overwrite to 1
        canonical_path: CanonicalPath::new("pkg::A::run"),
        name: "run".into(),
        kind: FunctionKind::InherentMethod { owner: id0 },
        role: FunctionRole::Helper,
        body: SemanticBody::empty(),
        provenance: test_provenance("identity"),
    }));
    assert_eq!(id1, ItemId(1));

    let id2 = store.push(Item::Enum(EnumDef {
        id: ItemId(7), // wrong -- store must overwrite to 2
        canonical_path: CanonicalPath::new("pkg::Status"),
        name: "Status".into(),
        variants: vec![],
        provenance: test_provenance("identity"),
    }));
    assert_eq!(id2, ItemId(2));

    let id3 = store.push(Item::Impl(ImplDef {
        id: ItemId(100), // wrong -- store must overwrite to 3
        canonical_path: CanonicalPath::new("pkg::<impl A>"),
        self_type: TypeRef::Item { id: id0 },
        trait_ref: None,
        methods: vec![id1],
        constants: vec![],
        provenance: test_provenance("identity"),
    }));
    assert_eq!(id3, ItemId(3));

    let id4 = store.push(Item::Constant(ConstantDef {
        id: ItemId(55), // wrong -- store must overwrite to 4
        canonical_path: CanonicalPath::new("pkg::A::MAX"),
        name: "MAX".into(),
        owner: Some(id0),
        ty: TypeRef::Primitive {
            ty: PrimitiveType::U32,
        },
        value: Some(ConstValue::Integer(256)),
        stable_name: "A_MAX".into(),
        provenance: test_provenance("identity"),
    }));
    assert_eq!(id4, ItemId(4));

    // Every retrieved item's embedded id must match its store-assigned id.
    assert_eq!(store.get(id0).unwrap().id(), id0);
    assert_eq!(store.get(id1).unwrap().id(), id1);
    assert_eq!(store.get(id2).unwrap().id(), id2);
    assert_eq!(store.get(id3).unwrap().id(), id3);
    assert_eq!(store.get(id4).unwrap().id(), id4);
}

// ---------------------------------------------------------------------------
// Test 2: Module.items references items through ItemId
// ---------------------------------------------------------------------------

#[test]
fn module_items_reference_through_item_id() {
    let mut store = ItemStore::new();

    let struct_id = store.push(Item::Struct(StructDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::driver::MyDriver"),
        name: "MyDriver".into(),
        fields: vec![],
        attributes: vec![],
        provenance: test_provenance("identity"),
    }));

    let fn_id = store.push(Item::Function(FunctionDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::driver::MyDriver::init"),
        name: "init".into(),
        kind: FunctionKind::InherentMethod { owner: struct_id },
        role: FunctionRole::Helper,
        body: SemanticBody::empty(),
        provenance: test_provenance("identity"),
    }));

    let module = Module {
        name: "driver".into(),
        canonical_path: CanonicalPath::new("pkg::driver"),
        items: vec![struct_id, fn_id],
        provenance: test_provenance("identity"),
    };

    assert_eq!(module.items.len(), 2);

    // Resolve each item through the store and verify embedded id coherence.
    let first = store.get(module.items[0]).unwrap();
    assert!(matches!(first, Item::Struct(_)));
    assert_eq!(
        first.id(),
        struct_id,
        "embedded id must match store-assigned id"
    );

    let second = store.get(module.items[1]).unwrap();
    assert!(matches!(second, Item::Function(_)));
    assert_eq!(
        second.id(),
        fn_id,
        "embedded id must match store-assigned id"
    );
}

// ---------------------------------------------------------------------------
// Test 3: ImplDef.methods and ImplDef.constants resolve through ItemId
// ---------------------------------------------------------------------------

#[test]
fn impl_methods_and_constants_resolve() {
    let mut store = ItemStore::new();

    let driver_id = store.push(Item::Struct(StructDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::driver::MyDriver"),
        name: "MyDriver".into(),
        fields: vec![],
        attributes: vec![ItemAttribute::Driver],
        provenance: test_provenance("identity"),
    }));

    let init_id = store.push(Item::Function(FunctionDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::driver::MyDriver::init"),
        name: "init".into(),
        kind: FunctionKind::TraitMethod {
            trait_ref: TraitRef {
                name: "Lifecycle".into(),
                path: "regseq::lifecycle::Lifecycle".into(),
            },
            owner: driver_id,
        },
        role: FunctionRole::Lifecycle {
            role: LifecycleRole::Init,
        },
        body: SemanticBody::empty(),
        provenance: test_provenance("identity"),
    }));

    let const_id = store.push(Item::Constant(ConstantDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::driver::MyDriver::DEFAULT_SPEED"),
        name: "DEFAULT_SPEED".into(),
        owner: Some(driver_id),
        ty: TypeRef::Primitive {
            ty: PrimitiveType::U32,
        },
        value: Some(ConstValue::Integer(115_200)),
        stable_name: "MY_DRIVER_DEFAULT_SPEED".into(),
        provenance: test_provenance("identity"),
    }));

    let impl_def = ImplDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::driver::<impl Lifecycle for MyDriver>"),
        self_type: TypeRef::Item { id: driver_id },
        trait_ref: Some(TraitRef {
            name: "Lifecycle".into(),
            path: "regseq::lifecycle::Lifecycle".into(),
        }),
        methods: vec![init_id],
        constants: vec![const_id],
        provenance: test_provenance("identity"),
    };

    // Methods resolve to FunctionDef with coherent id.
    let method = store.get(impl_def.methods[0]).unwrap();
    assert_eq!(
        method.id(),
        init_id,
        "embedded id must match store-assigned id"
    );
    match method {
        Item::Function(f) => assert_eq!(f.name, "init"),
        _ => panic!("expected Function item"),
    }

    // Constants resolve to ConstantDef with coherent id.
    let constant = store.get(impl_def.constants[0]).unwrap();
    assert_eq!(
        constant.id(),
        const_id,
        "embedded id must match store-assigned id"
    );
    match constant {
        Item::Constant(c) => {
            assert_eq!(c.name, "DEFAULT_SPEED");
            assert_eq!(c.owner, Some(driver_id));
        }
        _ => panic!("expected Constant item"),
    }
}

// ---------------------------------------------------------------------------
// Test 4: Item-level CanonicalPath lookup works deterministically
// ---------------------------------------------------------------------------

#[test]
fn item_level_canonical_path_lookup() {
    let mut table = CrossLinkTable::new();

    let struct_id = ItemId(0);
    let fn_id = ItemId(1);
    let const_id = ItemId(2);

    table.insert_item(CanonicalPath::new("pkg::driver::MyDriver"), struct_id);
    table.insert_item(CanonicalPath::new("pkg::driver::MyDriver::init"), fn_id);
    table.insert_item(
        CanonicalPath::new("pkg::driver::MyDriver::DEFAULT_SPEED"),
        const_id,
    );

    assert_eq!(
        table.resolve_item(&CanonicalPath::new("pkg::driver::MyDriver")),
        Some(struct_id)
    );
    assert_eq!(
        table.resolve_item(&CanonicalPath::new("pkg::driver::MyDriver::init")),
        Some(fn_id)
    );
    assert_eq!(
        table.resolve_item(&CanonicalPath::new("pkg::driver::MyDriver::DEFAULT_SPEED")),
        Some(const_id)
    );

    // Missing paths return None.
    assert_eq!(
        table.resolve_item(&CanonicalPath::new("pkg::nonexistent")),
        None
    );

    assert_eq!(table.item_count(), 3);
}

// ---------------------------------------------------------------------------
// Test 5: Field-level CanonicalPath lookup works deterministically
// ---------------------------------------------------------------------------

#[test]
fn field_level_canonical_path_lookup() {
    let mut table = CrossLinkTable::new();

    let struct_id = ItemId(0);

    table.insert_field(
        CanonicalPath::new("pkg::driver::MyDriver::regs"),
        struct_id,
        0,
    );
    table.insert_field(
        CanonicalPath::new("pkg::driver::MyDriver::clock"),
        struct_id,
        1,
    );

    assert_eq!(
        table.resolve_field(&CanonicalPath::new("pkg::driver::MyDriver::regs")),
        Some((struct_id, 0))
    );
    assert_eq!(
        table.resolve_field(&CanonicalPath::new("pkg::driver::MyDriver::clock")),
        Some((struct_id, 1))
    );

    // Missing field paths return None.
    assert_eq!(
        table.resolve_field(&CanonicalPath::new("pkg::driver::MyDriver::missing")),
        None
    );

    assert_eq!(table.field_count(), 2);
}

// ---------------------------------------------------------------------------
// Test 6: Overlapping short names are disambiguated through canonical identity
// ---------------------------------------------------------------------------

#[test]
fn overlapping_short_names_disambiguated() {
    let mut store = ItemStore::new();
    let mut table = CrossLinkTable::new();

    // Two structs both named "Config" in different modules.
    let config_a_id = store.push(Item::Struct(StructDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::mod_a::Config"),
        name: "Config".into(),
        fields: vec![StructFieldDef {
            canonical_path: CanonicalPath::new("pkg::mod_a::Config::rate"),
            name: "rate".into(),
            ty: TypeRef::Primitive {
                ty: PrimitiveType::U32,
            },
            attributes: vec![],
        }],
        attributes: vec![],
        provenance: test_provenance("identity"),
    }));

    let config_b_id = store.push(Item::Struct(StructDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::mod_b::Config"),
        name: "Config".into(),
        fields: vec![StructFieldDef {
            canonical_path: CanonicalPath::new("pkg::mod_b::Config::rate"),
            name: "rate".into(),
            ty: TypeRef::Primitive {
                ty: PrimitiveType::U32,
            },
            attributes: vec![],
        }],
        attributes: vec![],
        provenance: test_provenance("identity"),
    }));

    // Both resolve by distinct canonical paths despite same short name.
    table.insert_item(CanonicalPath::new("pkg::mod_a::Config"), config_a_id);
    table.insert_item(CanonicalPath::new("pkg::mod_b::Config"), config_b_id);

    assert_ne!(config_a_id, config_b_id);

    assert_eq!(
        table.resolve_item(&CanonicalPath::new("pkg::mod_a::Config")),
        Some(config_a_id)
    );
    assert_eq!(
        table.resolve_item(&CanonicalPath::new("pkg::mod_b::Config")),
        Some(config_b_id)
    );

    // Fields also disambiguated despite same short names.
    table.insert_field(
        CanonicalPath::new("pkg::mod_a::Config::rate"),
        config_a_id,
        0,
    );
    table.insert_field(
        CanonicalPath::new("pkg::mod_b::Config::rate"),
        config_b_id,
        0,
    );

    assert_eq!(
        table.resolve_field(&CanonicalPath::new("pkg::mod_a::Config::rate")),
        Some((config_a_id, 0))
    );
    assert_eq!(
        table.resolve_field(&CanonicalPath::new("pkg::mod_b::Config::rate")),
        Some((config_b_id, 0))
    );
}

// ---------------------------------------------------------------------------
// Test 7: Associated constants resolve without short-name matching
// ---------------------------------------------------------------------------

#[test]
fn associated_constants_resolve_without_short_name() {
    let mut store = ItemStore::new();
    let mut table = CrossLinkTable::new();

    let driver_a_id = store.push(Item::Struct(StructDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::driver_a::DriverA"),
        name: "DriverA".into(),
        fields: vec![],
        attributes: vec![],
        provenance: test_provenance("identity"),
    }));

    let driver_b_id = store.push(Item::Struct(StructDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::driver_b::DriverB"),
        name: "DriverB".into(),
        fields: vec![],
        attributes: vec![],
        provenance: test_provenance("identity"),
    }));

    // Both have DEFAULT_SPEED with same short name, different owners.
    let const_a_id = store.push(Item::Constant(ConstantDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::driver_a::DriverA::DEFAULT_SPEED"),
        name: "DEFAULT_SPEED".into(),
        owner: Some(driver_a_id),
        ty: TypeRef::Primitive {
            ty: PrimitiveType::U32,
        },
        value: Some(ConstValue::Integer(9600)),
        stable_name: "DRIVER_A_DEFAULT_SPEED".into(),
        provenance: test_provenance("identity"),
    }));

    let const_b_id = store.push(Item::Constant(ConstantDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::driver_b::DriverB::DEFAULT_SPEED"),
        name: "DEFAULT_SPEED".into(),
        owner: Some(driver_b_id),
        ty: TypeRef::Primitive {
            ty: PrimitiveType::U32,
        },
        value: Some(ConstValue::Integer(115_200)),
        stable_name: "DRIVER_B_DEFAULT_SPEED".into(),
        provenance: test_provenance("identity"),
    }));

    table.insert_item(
        CanonicalPath::new("pkg::driver_a::DriverA::DEFAULT_SPEED"),
        const_a_id,
    );
    table.insert_item(
        CanonicalPath::new("pkg::driver_b::DriverB::DEFAULT_SPEED"),
        const_b_id,
    );

    // Resolve by canonical path, not by short name.
    let resolved_a = table
        .resolve_item(&CanonicalPath::new("pkg::driver_a::DriverA::DEFAULT_SPEED"))
        .unwrap();
    let resolved_b = table
        .resolve_item(&CanonicalPath::new("pkg::driver_b::DriverB::DEFAULT_SPEED"))
        .unwrap();

    assert_ne!(resolved_a, resolved_b);

    match store.get(resolved_a).unwrap() {
        Item::Constant(c) => {
            assert_eq!(c.owner, Some(driver_a_id));
            assert_eq!(c.value, Some(ConstValue::Integer(9600)));
            assert_eq!(c.stable_name, "DRIVER_A_DEFAULT_SPEED");
        }
        _ => panic!("expected Constant"),
    }

    match store.get(resolved_b).unwrap() {
        Item::Constant(c) => {
            assert_eq!(c.owner, Some(driver_b_id));
            assert_eq!(c.value, Some(ConstValue::Integer(115_200)));
            assert_eq!(c.stable_name, "DRIVER_B_DEFAULT_SPEED");
        }
        _ => panic!("expected Constant"),
    }
}

// ---------------------------------------------------------------------------
// Test 8: Schema remains device-neutral and OS-neutral
// ---------------------------------------------------------------------------

#[test]
fn schema_is_device_neutral() {
    // Build a complete hierarchy using fully generic names -- no PL011,
    // no Linux, no UART, no Zephyr references in type construction.
    let mut store = ItemStore::new();

    let driver_id = store.push(Item::Struct(StructDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("my_pkg::driver::MyDriver"),
        name: "MyDriver".into(),
        fields: vec![
            StructFieldDef {
                canonical_path: CanonicalPath::new("my_pkg::driver::MyDriver::regs"),
                name: "regs".into(),
                ty: TypeRef::Named {
                    name: "Mmio<MyRegisterBlock>".into(),
                },
                attributes: vec![FieldAttribute::Mmio {
                    id: "primary".into(),
                }],
            },
            StructFieldDef {
                canonical_path: CanonicalPath::new("my_pkg::driver::MyDriver::clk"),
                name: "clk".into(),
                ty: TypeRef::Named {
                    name: "Option<Clock>".into(),
                },
                attributes: vec![FieldAttribute::Clock {
                    id: "bus_clk".into(),
                }],
            },
        ],
        attributes: vec![ItemAttribute::Driver],
        provenance: test_provenance("identity"),
    }));

    let init_id = store.push(Item::Function(FunctionDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("my_pkg::driver::MyDriver::init"),
        name: "init".into(),
        kind: FunctionKind::TraitMethod {
            trait_ref: TraitRef {
                name: "Lifecycle".into(),
                path: "regseq::lifecycle::Lifecycle".into(),
            },
            owner: driver_id,
        },
        role: FunctionRole::Lifecycle {
            role: LifecycleRole::Init,
        },
        body: SemanticBody::empty(),
        provenance: test_provenance("identity"),
    }));

    let helper_id = store.push(Item::Function(FunctionDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("my_pkg::driver::MyDriver::do_reset"),
        name: "do_reset".into(),
        kind: FunctionKind::InherentMethod { owner: driver_id },
        role: FunctionRole::Helper,
        body: SemanticBody::empty(),
        provenance: test_provenance("identity"),
    }));

    let module = Module {
        name: "driver".into(),
        canonical_path: CanonicalPath::new("my_pkg::driver"),
        items: vec![driver_id, init_id, helper_id],
        provenance: test_provenance("identity"),
    };

    // Verify the hierarchy is well-formed without any device-specific types.
    assert_eq!(module.items.len(), 3);
    assert_eq!(store.len(), 3);

    // All items resolve.
    for &id in &module.items {
        assert!(store.get(id).is_some());
    }
}

// ---------------------------------------------------------------------------
// Test 9: No device-specific type names or required identities
// ---------------------------------------------------------------------------

#[test]
fn no_device_specific_type_names() {
    // Construct a non-PL011, non-UART example (e.g. network driver)
    // proving the schema accepts arbitrary device-class names.
    let mut store = ItemStore::new();

    let driver_id = store.push(Item::Struct(StructDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("e1000_net::driver::E1000Driver"),
        name: "E1000Driver".into(),
        fields: vec![],
        attributes: vec![ItemAttribute::Driver],
        provenance: test_provenance("identity"),
    }));

    let _tx_id = store.push(Item::Function(FunctionDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("e1000_net::driver::E1000Driver::tx_packet"),
        name: "tx_packet".into(),
        kind: FunctionKind::TraitMethod {
            trait_ref: TraitRef {
                name: "NetworkIo".into(),
                path: "regseq::net::NetworkIo".into(),
            },
            owner: driver_id,
        },
        role: FunctionRole::DeviceClassCallback {
            class: DeviceClassKind::Network,
            callback: DeviceCallbackRole::Named("tx_packet".into()),
        },
        body: SemanticBody::empty(),
        provenance: test_provenance("identity"),
    }));

    let _isr_id = store.push(Item::Function(FunctionDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("e1000_net::driver::E1000Driver::handle_irq"),
        name: "handle_irq".into(),
        kind: FunctionKind::InherentMethod { owner: driver_id },
        role: FunctionRole::Isr {
            binding: "irq".into(),
        },
        body: SemanticBody::empty(),
        provenance: test_provenance("identity"),
    }));

    // Schema accepted arbitrary names without PL011/UART/Linux references.
    assert_eq!(store.len(), 3);
}

// ---------------------------------------------------------------------------
// Additional: Serde JSON round-trip
// ---------------------------------------------------------------------------

#[test]
fn serde_json_round_trip() {
    let mut store = ItemStore::new();

    let struct_id = store.push(Item::Struct(StructDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::MyType"),
        name: "MyType".into(),
        fields: vec![StructFieldDef {
            canonical_path: CanonicalPath::new("pkg::MyType::value"),
            name: "value".into(),
            ty: TypeRef::Primitive {
                ty: PrimitiveType::U32,
            },
            attributes: vec![],
        }],
        attributes: vec![ItemAttribute::ReprC],
        provenance: test_provenance("identity"),
    }));

    let _fn_id = store.push(Item::Function(FunctionDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::MyType::run"),
        name: "run".into(),
        kind: FunctionKind::InherentMethod { owner: struct_id },
        role: FunctionRole::Helper,
        body: SemanticBody::empty(),
        provenance: test_provenance("identity"),
    }));

    let enum_id = store.push(Item::Enum(EnumDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::Status"),
        name: "Status".into(),
        variants: vec![
            EnumVariantDef {
                name: "Ok".into(),
                discriminant: Some(0),
                fields: vec![],
            },
            EnumVariantDef {
                name: "Error".into(),
                discriminant: Some(1),
                fields: vec![],
            },
        ],
        provenance: test_provenance("identity"),
    }));

    let _const_id = store.push(Item::Constant(ConstantDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::MyType::MAX_SIZE"),
        name: "MAX_SIZE".into(),
        owner: Some(struct_id),
        ty: TypeRef::Primitive {
            ty: PrimitiveType::Usize,
        },
        value: Some(ConstValue::Integer(4096)),
        stable_name: "MY_TYPE_MAX_SIZE".into(),
        provenance: test_provenance("identity"),
    }));

    let _impl_id = store.push(Item::Impl(ImplDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::<impl MyType>"),
        self_type: TypeRef::Item { id: struct_id },
        trait_ref: None,
        methods: vec![ItemId(1)],
        constants: vec![ItemId(3)],
        provenance: test_provenance("identity"),
    }));

    let module = Module {
        name: "pkg".into(),
        canonical_path: CanonicalPath::new("pkg"),
        items: vec![struct_id, ItemId(1), enum_id, ItemId(3), ItemId(4)],
        provenance: test_provenance("identity"),
    };

    // Round-trip the store.
    let json = serde_json::to_string_pretty(&store).expect("serialize store");
    let store2: ItemStore = serde_json::from_str(&json).expect("deserialize store");
    assert_eq!(store, store2);

    // Round-trip the module.
    let json = serde_json::to_string_pretty(&module).expect("serialize module");
    let module2: Module = serde_json::from_str(&json).expect("deserialize module");
    assert_eq!(module, module2);

    // Round-trip the cross-link table.
    let mut table = CrossLinkTable::new();
    table.insert_item(CanonicalPath::new("pkg::MyType"), struct_id);
    table.insert_item(CanonicalPath::new("pkg::Status"), enum_id);
    table.insert_field(CanonicalPath::new("pkg::MyType::value"), struct_id, 0);

    let json = serde_json::to_string_pretty(&table).expect("serialize table");
    let table2: CrossLinkTable = serde_json::from_str(&json).expect("deserialize table");
    assert_eq!(table, table2);

    // Round-trip InterruptClassDef.
    let icd = InterruptClassDef {
        name: "rx".into(),
        mask_value: 0x10,
    };
    let json = serde_json::to_string(&icd).expect("serialize icd");
    let icd2: InterruptClassDef = serde_json::from_str(&json).expect("deserialize icd");
    assert_eq!(icd, icd2);
}

// ---------------------------------------------------------------------------
// Additional: FunctionKind and FunctionRole are independent
// ---------------------------------------------------------------------------

#[test]
fn function_kind_and_role_are_independent() {
    let driver_id = ItemId(0);

    // InherentMethod can be Helper.
    let helper = FunctionDef {
        id: ItemId(1),
        canonical_path: CanonicalPath::new("pkg::MyDriver::set_baud_rate"),
        name: "set_baud_rate".into(),
        kind: FunctionKind::InherentMethod { owner: driver_id },
        role: FunctionRole::Helper,
        body: SemanticBody::empty(),
        provenance: test_provenance("identity"),
    };
    assert!(matches!(helper.kind, FunctionKind::InherentMethod { .. }));
    assert!(matches!(helper.role, FunctionRole::Helper));

    // TraitMethod can be Lifecycle(Init).
    let init = FunctionDef {
        id: ItemId(2),
        canonical_path: CanonicalPath::new("pkg::MyDriver::init"),
        name: "init".into(),
        kind: FunctionKind::TraitMethod {
            trait_ref: TraitRef {
                name: "Lifecycle".into(),
                path: "regseq::lifecycle::Lifecycle".into(),
            },
            owner: driver_id,
        },
        role: FunctionRole::Lifecycle {
            role: LifecycleRole::Init,
        },
        body: SemanticBody::empty(),
        provenance: test_provenance("identity"),
    };
    assert!(matches!(init.kind, FunctionKind::TraitMethod { .. }));
    assert!(matches!(
        init.role,
        FunctionRole::Lifecycle {
            role: LifecycleRole::Init
        }
    ));

    // InherentMethod can be Isr.
    let isr = FunctionDef {
        id: ItemId(3),
        canonical_path: CanonicalPath::new("pkg::MyDriver::handle_irq"),
        name: "handle_irq".into(),
        kind: FunctionKind::InherentMethod { owner: driver_id },
        role: FunctionRole::Isr {
            binding: "irq".into(),
        },
        body: SemanticBody::empty(),
        provenance: test_provenance("identity"),
    };
    assert!(matches!(isr.kind, FunctionKind::InherentMethod { .. }));
    assert!(matches!(isr.role, FunctionRole::Isr { .. }));

    // TraitMethod can be DeviceClassCallback.
    let callback = FunctionDef {
        id: ItemId(4),
        canonical_path: CanonicalPath::new("pkg::MyDriver::tx_byte"),
        name: "tx_byte".into(),
        kind: FunctionKind::TraitMethod {
            trait_ref: TraitRef {
                name: "Serial".into(),
                path: "regseq::serial::Serial".into(),
            },
            owner: driver_id,
        },
        role: FunctionRole::DeviceClassCallback {
            class: DeviceClassKind::Serial,
            callback: DeviceCallbackRole::TxByte,
        },
        body: SemanticBody::empty(),
        provenance: test_provenance("identity"),
    };
    assert!(matches!(callback.kind, FunctionKind::TraitMethod { .. }));
    assert!(matches!(
        callback.role,
        FunctionRole::DeviceClassCallback { .. }
    ));
}
