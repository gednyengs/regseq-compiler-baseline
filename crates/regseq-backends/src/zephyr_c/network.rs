//! Zephyr lowering for the locked network-interface element.
//!
//! Zephyr interfaces are link-time-created, so runtime publication remains an
//! explicit negative capability. Native packet values are `net_pkt *`; their
//! layout and queueing never enter portable IR.

use regseq_model::{InstanceId, InstanceRef, ItemId, RegseqModel, TyId};

use super::body::LowerCx;
use super::types;
use super::EmitError;

pub(super) const E1000_DESCRIPTOR_RING_SIZE: u64 = 8;
pub(super) const E1000_DESCRIPTOR_RING_BYTES: u64 = E1000_DESCRIPTOR_RING_SIZE * 16;

pub(super) struct Emission {
    pub data_fields: String,
    pub text: String,
    pub api: String,
    pub publishes_interface: bool,
}

impl Emission {
    fn absent() -> Self {
        Self {
            data_fields: String::new(),
            text: String::new(),
            api: String::new(),
            publishes_interface: false,
        }
    }
}

struct Op {
    symbol: String,
    inputs: Vec<TyId>,
    output: TyId,
}

pub(super) fn descriptor_types(model: &RegseqModel, driver: ItemId, drv: &str) -> String {
    if !super::dma::descriptor_datapath(model, driver) {
        return String::new();
    }
    let upper = drv.to_ascii_uppercase();
    let ring_size = E1000_DESCRIPTOR_RING_SIZE;
    format!(
        r#"/* Intel 82540EM legacy layouts: rx_tx_operations.md 47-100, 487-555.
 * Stock Zephyr Kconfig.e1000 defaults both queues to 8; eth_e1000_priv.h:84-102. */
#define {upper}_DESCRIPTOR_RING_SIZE {ring_size}u
#define {upper}_RX_BUFFER_SIZE 2048u

struct {drv}_legacy_tx_descriptor {{
	uint64_t buffer_address;
	uint16_t length;
	uint8_t checksum_offset;
	uint8_t command;
	volatile uint8_t status;
	uint8_t checksum_start;
	uint16_t special;
}} __packed;

struct {drv}_legacy_rx_descriptor {{
	uint64_t buffer_address;
	volatile uint16_t length;
	volatile uint16_t checksum;
	volatile uint8_t status;
	volatile uint8_t errors;
	volatile uint16_t special;
}} __packed;

_Static_assert(sizeof(struct {drv}_legacy_tx_descriptor) == 16, "legacy TX descriptor size");
_Static_assert(sizeof(struct {drv}_legacy_rx_descriptor) == 16, "legacy RX descriptor size");

"#
    )
}

fn op(cx: &LowerCx<'_>, value: &Option<InstanceRef>) -> Result<Option<Op>, EmitError> {
    let Some(reference) = value else {
        return Ok(None);
    };
    let id = resolve_instance_ref(cx.model, *reference).ok_or_else(|| {
        EmitError::Driver("network operation does not resolve to an instance".into())
    })?;
    let regseq_model::InstanceKind::Function(data) = &cx.model.instances[id].kind else {
        return Err(EmitError::Driver(
            "network operation resolved to a non-function instance".into(),
        ));
    };
    let sig = cx.model.sigs.resolve(data.fn_sig);
    Ok(Some(Op {
        symbol: cx.fn_symbols[&id].clone(),
        inputs: sig.inputs.clone(),
        output: sig.output,
    }))
}

pub(super) fn roots(
    model: &RegseqModel,
    driver: ItemId,
) -> Result<Vec<(&'static str, InstanceId)>, EmitError> {
    let Some(interface) = model.network_interface.drivers.get(&driver) else {
        return Ok(Vec::new());
    };
    let refs: [(&str, Option<InstanceRef>); 12] = [
        ("set_published", interface.ops.set_published),
        ("set_admin_enabled", interface.ops.set_admin_enabled),
        ("transmit", interface.ops.transmit),
        ("receive", interface.ops.receive),
        ("report_carrier", interface.ops.report_carrier),
        ("report_dormant", interface.ops.report_dormant),
        (
            "query_operational_state",
            interface.ops.query_operational_state,
        ),
        ("get_property", interface.ops.get_property),
        ("set_property", interface.ops.set_property),
        ("read_counters", interface.ops.read_counters),
        ("register_observer", interface.ops.register_observer),
        ("unregister_observer", interface.ops.unregister_observer),
    ];
    refs.into_iter()
        .filter_map(|(name, reference)| reference.map(|reference| (name, reference)))
        .map(|(name, reference)| {
            resolve_instance_ref(model, reference)
                .map(|id| (name, id))
                .ok_or_else(|| {
                    EmitError::Driver(format!(
                        "network operation `{name}` does not resolve to an instance"
                    ))
                })
        })
        .collect()
}

pub(super) fn emit(cx: &mut LowerCx<'_>, driver: ItemId, drv: &str) -> Result<Emission, EmitError> {
    let Some(interface) = cx.model.network_interface.drivers.get(&driver) else {
        return Ok(Emission::absent());
    };
    if interface.ops.set_published.is_some() {
        return Err(EmitError::Rejected(
            "network-interface runtime publication is unsupported on Zephyr: net_if instances are link-time-created; no successful set_published lowering is synthesized".into(),
        ));
    }
    for include in [
        "<errno.h>",
        "<zephyr/net/ethernet.h>",
        "<zephyr/net/net_core.h>",
        "<zephyr/net/net_if.h>",
        "<zephyr/net/net_pkt.h>",
    ] {
        cx.extra_includes.insert(include);
    }

    let set_admin = op(cx, &interface.ops.set_admin_enabled)?;
    let transmit = op(cx, &interface.ops.transmit)?;
    let set_property = op(cx, &interface.ops.set_property)?;
    let counters = op(cx, &interface.ops.read_counters)?;

    let mut out = format!(
        r#"/* RSEQ-NIM: Zephyr net_if is link-time-created; packet layout stays native. */
static int __unused {drv}_net_set_admin(struct net_if *iface, bool enabled)
{{
	return enabled ? net_if_up(iface) : net_if_down(iface);
}}

static int __unused {drv}_net_send_data(struct net_if *iface,
		struct net_pkt *packet)
{{
	return net_if_send_data(iface, packet);
}}

static void __unused {drv}_net_receive(struct net_if *iface,
		struct net_pkt *packet)
{{
	(void)net_recv_data(iface, packet); /* no normalized receive verdict */
}}

static void __unused {drv}_net_report_carrier(struct net_if *iface, bool present)
{{
	if (present)
		net_if_carrier_on(iface);
	else
		net_if_carrier_off(iface);
}}

static int __unused {drv}_net_report_dormant(struct net_if *iface, bool dormant)
{{
	if (dormant)
		net_if_dormant_on(iface);
	else
		net_if_dormant_off(iface);
	return 0;
}}

static enum net_if_oper_state __unused {drv}_net_operational_state(
		struct net_if *iface)
{{
	return net_if_is_up(iface) ? NET_IF_OPER_UP : NET_IF_OPER_DOWN;
}}

static int __unused {drv}_net_index(struct net_if *iface)
{{
	return net_if_get_by_iface(iface);
}}

static uint16_t __unused {drv}_net_mtu(struct net_if *iface)
{{
	return net_if_get_mtu(iface);
}}

static int __unused {drv}_net_set_mtu(struct net_if *iface, uint16_t mtu)
{{
	net_if_set_mtu(iface, mtu);
	return 0;
}}

static const struct net_linkaddr *__unused {drv}_net_link_address(
		struct net_if *iface)
{{
	return net_if_get_link_addr(iface);
}}

"#,
    );

    let mac_init = if super::dma::descriptor_datapath(cx.model, driver) {
        "\t/* RAR0 was synchronized into authored state before net_if publication. */\n\t{\n\t\tuint8_t __mac[6] = {\n\t\t\t(uint8_t)data->inner.mac_low, (uint8_t)(data->inner.mac_low >> 8),\n\t\t\t(uint8_t)(data->inner.mac_low >> 16), (uint8_t)(data->inner.mac_low >> 24),\n\t\t\t(uint8_t)data->inner.mac_high, (uint8_t)(data->inner.mac_high >> 8),\n\t\t};\n\n\t\t(void)net_if_set_link_addr(iface, __mac, sizeof(__mac), NET_LINK_ETHERNET);\n\t}\n"
    } else {
        ""
    };
    out.push_str(&format!(
        "static void {drv}_net_iface_init(struct net_if *iface)\n{{\n\tconst struct device *dev = net_if_get_device(iface);\n\tstruct {drv}_data *data = dev->data;\n\n\tdata->__rseq_net_iface = iface;\n\tethernet_init(iface);\n{mac_init}\t/* Preserve authored admin state until an explicit net_if_up(). */\n\tnet_if_flag_set(iface, NET_IF_NO_AUTO_START);\n}}\n\n"
    ));
    if let Some(op) = &set_admin {
        let multicast_prepare = if super::dma::descriptor_datapath(cx.model, driver) {
            if let Some(property_op) = &set_property {
                let property = types::c_type(cx.model, &mut cx.names, property_op.inputs[1])?;
                let value = types::c_type(cx.model, &mut cx.names, property_op.inputs[2])?;
                format!(
                    "\t/* IPv6 ND requires multicast admission. Stock eth_e1000.c:331\n\t * enables RCTL_MPE; select the already-authored AllMulticast mode. */\n\t{property} property = {{ ._raw = {{ 8 }} }};\n\t{value} value = {{0}};\n\tint ret;\n\n\t/* Locked network-surface variant order: ReceiveFilter(AllMulticast). */\n\tvalue._raw[0] = 6;\n\tvalue._raw[1] = 1;\n\tret = (int){sym}(&data->inner, property, value);\n\tif (ret != 0)\n\t\treturn ret;\n",
                    sym = property_op.symbol,
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        out.push_str(&format!(
            "static int {drv}_net_start(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n{multicast_prepare}\n\treturn (int){sym}(&data->inner, true);\n}}\n\nstatic int {drv}_net_stop(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\treturn (int){sym}(&data->inner, false);\n}}\n\n",
            sym = op.symbol,
        ));
    }
    if let Some(op) = &transmit {
        let packet = types::c_type(cx.model, &mut cx.names, op.inputs[1])?;
        let verdict = types::c_type(cx.model, &mut cx.names, op.output)?;
        out.push_str(&format!(
            "static int {drv}_net_send(const struct device *dev, struct net_pkt *packet)\n{{\n\tstruct {drv}_data *data = dev->data;\n\t{packet} native = {{ .identity = (size_t)(uintptr_t)packet }};\n\t{verdict} verdict = {sym}(&data->inner, native);\n\n\treturn verdict._raw[0] == 0 ? 0 : -EBUSY;\n}}\n\n",
            sym = op.symbol,
        ));
    } else {
        out.push_str(&format!(
            "static int {drv}_net_send(const struct device *dev, struct net_pkt *packet)\n{{\n\t(void)dev;\n\t(void)packet;\n\treturn -ENOTSUP;\n}}\n\n"
        ));
    }
    if let Some(op) = &counters {
        let counter_ty = types::c_type(cx.model, &mut cx.names, op.output)?;
        out.push_str(&format!(
            "#if defined(CONFIG_NET_STATISTICS_ETHERNET)\n/* Stock eth_e1000.c:289-305 publishes its net_stats_eth through get_stats. */\nstatic struct net_stats_eth *{drv}_net_get_stats(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\t{counter_ty} counters = {sym}(&data->inner);\n\n\tdata->__rseq_net_stats.pkts.rx = (net_stats_t)counters.rx_packets;\n\tdata->__rseq_net_stats.pkts.tx = (net_stats_t)counters.tx_packets;\n\tdata->__rseq_net_stats.bytes.received = counters.rx_bytes;\n\tdata->__rseq_net_stats.bytes.sent = counters.tx_bytes;\n\tdata->__rseq_net_stats.errors.rx = (net_stats_t)counters.rx_errors;\n\tdata->__rseq_net_stats.errors.tx = (net_stats_t)counters.tx_errors;\n\tdata->__rseq_net_stats.tx_dropped = (net_stats_t)counters.tx_dropped;\n\treturn &data->__rseq_net_stats;\n}}\n#endif\n\n",
            sym = op.symbol,
        ));
    }
    out.push_str(&format!(
        "static const struct ethernet_api {drv}_net_api = {{\n\t.iface_api.init = {drv}_net_iface_init,\n"
    ));
    if counters.is_some() {
        out.push_str(&format!(
            "#if defined(CONFIG_NET_STATISTICS_ETHERNET)\n\t.get_stats = {drv}_net_get_stats,\n#endif\n"
        ));
    }
    if set_admin.is_some() {
        out.push_str(&format!(
            "\t.start = {drv}_net_start,\n\t.stop = {drv}_net_stop,\n"
        ));
    }
    out.push_str(&format!("\t.send = {drv}_net_send,\n}};\n\n"));
    for capability in interface.ops.capabilities() {
        out.push_str(&format!("/* RSEQ-NIM-CAPABILITY {capability} */\n"));
    }
    out.push_str("/* Native statistics use the standard Zephyr binding; no packet completion is fabricated. */\n\n");
    let mut data_fields = if super::dma::descriptor_datapath(cx.model, driver) {
        format!(
            "\tstruct net_if *__rseq_net_iface;\n\tsize_t __rseq_tx_lengths[{ring_size}];\n\tuint8_t __rseq_tx_buffers[{ring_size}][2048] __aligned(16);\n\tuint8_t __rseq_rx_buffers[{ring_size}][2048] __aligned(16);\n",
            ring_size = E1000_DESCRIPTOR_RING_SIZE,
        )
    } else {
        "\tstruct net_if *__rseq_net_iface;\n".to_string()
    };
    if counters.is_some() {
        data_fields.push_str(
            "#if defined(CONFIG_NET_STATISTICS_ETHERNET)\n\tstruct net_stats_eth __rseq_net_stats;\n#endif\n",
        );
    }
    Ok(Emission {
        data_fields,
        text: out,
        api: format!("&{drv}_net_api"),
        publishes_interface: true,
    })
}

fn resolve_instance_ref(model: &RegseqModel, target: InstanceRef) -> Option<InstanceId> {
    (0..model.instances.len())
        .map(|i| InstanceId::from_u32(i as u32))
        .find(|id| model.instance_ref_of(*id) == target)
}
