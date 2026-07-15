//! Zephyr lowering for the locked network-interface element.
//!
//! Zephyr interfaces are link-time-created, so runtime publication remains an
//! explicit negative capability. Native packet values are `net_pkt *`; their
//! layout and queueing never enter portable IR.

use regseq_model::{InstanceId, InstanceRef, ItemId, RegseqModel};

use super::body::LowerCx;
use super::EmitError;

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
        ("query_operational_state", interface.ops.query_operational_state),
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
                .ok_or_else(|| EmitError::Driver(format!("network operation `{name}` does not resolve to an instance")))
        })
        .collect()
}

pub(super) fn emit(
    cx: &mut LowerCx<'_>,
    driver: ItemId,
    drv: &str,
) -> Result<String, EmitError> {
    let Some(interface) = cx.model.network_interface.drivers.get(&driver) else {
        return Ok(String::new());
    };
    if interface.ops.set_published.is_some() {
        return Err(EmitError::Rejected(
            "network-interface runtime publication is unsupported on Zephyr: net_if instances are link-time-created; no successful set_published lowering is synthesized".into(),
        ));
    }
    for include in [
        "<zephyr/net/net_core.h>",
        "<zephyr/net/net_if.h>",
        "<zephyr/net/net_pkt.h>",
    ] {
        cx.extra_includes.insert(include);
    }

    let mut out = format!(
        r#"/* RSEQ-NIM: Zephyr net_if is link-time-created; packet layout stays native. */
static int __unused {drv}_net_set_admin(struct net_if *iface, bool enabled)
{{
	return enabled ? net_if_up(iface) : net_if_down(iface);
}}

static int __unused {drv}_net_transmit(struct net_if *iface,
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
    for capability in interface.ops.capabilities() {
        out.push_str(&format!("/* RSEQ-NIM-CAPABILITY {capability} */\n"));
    }
    out.push_str("/* Native statistics/event transport stay in the Zephyr binding; no packet completion is fabricated. */\n\n");
    Ok(out)
}

fn resolve_instance_ref(model: &RegseqModel, target: InstanceRef) -> Option<InstanceId> {
    (0..model.instances.len())
        .map(|i| InstanceId::from_u32(i as u32))
        .find(|id| model.instance_ref_of(*id) == target)
}
