#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

#[path = "support/regseq_model_harness.rs"]
mod harness;

use harness::{
    compile_and_build, driver_struct, find_driver_module, find_register_block, resolve_symbol,
};
use regseq_model::model::{Shape, UserDataKind};

const SPECIMEN: &str = r#"
#![no_std]

pub struct Clock;
pub struct ResetLine;
pub struct PinCtrl;
pub struct Interrupt;
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
pub struct ReadWrite<T, R> { _v: T, _r: core::marker::PhantomData<R> }

#[allow(non_snake_case)]
pub mod DR_REG { pub struct Register; }

#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct Regs {
    pub dr: ReadWrite<u32, DR_REG::Register>,
}

#[derive(Copy, Clone)]
pub struct Stats {
    pub count: u32,
}

#[rseq::driver(name = "pl011-like", compatible = ["arm,pl011"], bus = "amba")]
#[rseq::device_class(serial)]
pub struct Pl011Like {
    #[rseq::mmio(id = "uart_regs")]
    pub regs: Mmio<Regs>,
    #[rseq::interrupt(id = "uart_irq")]
    pub irq: Interrupt,
    #[rseq::clock(id = "uart_clk")]
    pub clock: Option<Clock>,
    #[rseq::reset(id = "uart_rst")]
    pub reset: Option<ResetLine>,
    #[rseq::pinctrl(id = "uart_pins")]
    pub pinctrl: Option<PinCtrl>,
    #[rseq::property(id = "current-speed", default = 115200, vendor_prefix = "acme")]
    pub current_speed: u32,
    #[rseq::property(id = "fifo-size", default = 32)]
    pub fifo_size: u32,
    pending_tx: Option<u8>,
    stats: Stats,
}
"#;

#[test]
fn driver_identity_and_resources_land_in_regseq_model() {
    let model = compile_and_build(SPECIMEN, "source_to_regseq_model_03_resources")
        .expect("specimen must build");
    let driver = driver_struct(&model);

    assert_eq!(resolve_symbol(&model, driver.name), "Pl011Like");
    assert_eq!(resolve_symbol(&model, driver.identity.name), "pl011-like");
    assert_eq!(driver.identity.compatible.len(), 1);
    assert_eq!(
        resolve_symbol(&model, driver.identity.compatible[0]),
        "arm,pl011"
    );
    assert_eq!(
        driver.identity.bus.map(|sym| resolve_symbol(&model, sym)),
        Some("amba")
    );
    assert_eq!(
        find_driver_module(&model)
            .type_defs
            .iter()
            .find(|td| resolve_symbol(&model, td.name) == "Pl011Like")
            .expect("driver TypeDef must exist")
            .attrs
            .entries
            .iter()
            .find(|entry| entry.key == "rseq::device_class")
            .map(|_| "present"),
        Some("present")
    );

    assert_eq!(driver.mmio.len(), 1);
    let mmio = &driver.mmio[0];
    assert_eq!(resolve_symbol(&model, mmio.field_name), "regs");
    assert_eq!(mmio.shape, Shape::Single);
    assert_eq!(resolve_symbol(&model, mmio.decl.mmio_id), "uart_regs");
    assert_eq!(
        resolve_symbol(&model, find_register_block(&model, "Regs").name),
        "Regs"
    );
    assert_eq!(
        mmio.decl.register_block,
        find_register_block(&model, "Regs").id
    );

    assert_eq!(driver.interrupts.len(), 1);
    assert_eq!(
        resolve_symbol(&model, driver.interrupts[0].decl.interrupt_id),
        "uart_irq"
    );

    assert_eq!(driver.clocks.len(), 1);
    assert_eq!(driver.clocks[0].shape, Shape::Optional);
    assert_eq!(
        resolve_symbol(&model, driver.clocks[0].decl.clock_id),
        "uart_clk"
    );

    assert_eq!(driver.resets.len(), 1);
    assert_eq!(driver.resets[0].shape, Shape::Optional);

    assert_eq!(driver.pinctrl.len(), 1);
    assert_eq!(driver.pinctrl[0].shape, Shape::Optional);
    assert_eq!(
        resolve_symbol(&model, driver.pinctrl[0].decl.pinctrl_id),
        "uart_pins"
    );
}

#[test]
fn driver_properties_and_user_data_use_model_shapes() {
    let model = compile_and_build(SPECIMEN, "source_to_regseq_model_03_fields")
        .expect("specimen must build");
    let driver = driver_struct(&model);

    assert_eq!(driver.properties.len(), 2);
    let current_speed = driver
        .properties
        .iter()
        .find(|prop| resolve_symbol(&model, prop.field_name) == "current_speed")
        .expect("current_speed property must exist");
    assert_eq!(
        resolve_symbol(&model, current_speed.property_id),
        "current-speed"
    );
    assert!(current_speed.required);
    assert_eq!(
        current_speed
            .vendor_prefix
            .map(|prefix| resolve_symbol(&model, prefix)),
        Some("acme")
    );

    let fifo_size = driver
        .properties
        .iter()
        .find(|prop| resolve_symbol(&model, prop.field_name) == "fifo_size")
        .expect("fifo_size property must exist");
    assert_eq!(resolve_symbol(&model, fifo_size.property_id), "fifo-size");
    assert!(fifo_size.required);

    assert_eq!(driver.user_data.len(), 2);
    let pending_tx = driver
        .user_data
        .iter()
        .find(|field| resolve_symbol(&model, field.field_name) == "pending_tx")
        .expect("pending_tx must remain user_data");
    assert!(
        matches!(pending_tx.kind, UserDataKind::Option { .. }),
        "pending_tx should lower as UserDataKind::Option"
    );

    let stats = driver
        .user_data
        .iter()
        .find(|field| resolve_symbol(&model, field.field_name) == "stats")
        .expect("stats must remain user_data");
    assert!(
        matches!(stats.kind, UserDataKind::ZeroInit { .. }),
        "Copy struct user data should lower as UserDataKind::ZeroInit"
    );
}
