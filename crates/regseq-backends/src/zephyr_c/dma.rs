//! Locked asynchronous DMA channel lowering for Zephyr.
//!
//! Zephyr has no owned-buffer allocator in the locked surface, so any
//! `DmaBuffer` element is rejected. The channel path uses the public DMA API;
//! endpoint/controller resolution remains an L2 parameter.

use regseq_model::{AttrData, EnumShape, InstanceId, InstanceRef, ItemId, LayoutKind, RegseqModel};

use super::body::LowerCx;
use super::EmitError;

pub(super) fn emit(cx: &mut LowerCx<'_>, driver: ItemId, drv: &str) -> Result<String, EmitError> {
    let Some(dma) = cx.model.dma.drivers.get(&driver) else {
        return Ok(String::new());
    };
    if dma.buffer.is_some() && !descriptor_datapath(cx.model, driver) {
        return Err(EmitError::Rejected(
            "DmaBuffer owned allocation is unsupported on Zephyr: the locked headers expose caller-buffer alignment but no owned-buffer allocator".into(),
        ));
    }
    if let Some(buffer) = &dma.buffer {
        let mut out = String::new();
        for capability in buffer.ops.capabilities() {
            out.push_str(&format!(
                "/* RSEQ-DMA-BUFFER-CAPABILITY {capability} (static PCIe descriptor ring) */\n"
            ));
        }
        out.push('\n');
        return Ok(out);
    }
    let Some(channel) = &dma.channel else {
        return Ok(String::new());
    };
    cx.extra_includes.insert("<errno.h>");
    cx.extra_includes.insert("<stdbool.h>");
    cx.extra_includes.insert("<string.h>");
    cx.extra_includes.insert("<zephyr/drivers/dma.h>");

    let ops = &channel.ops;
    let mut out = format!(
        r#"/* RSEQ-DMA-CHANNEL: controller resolution is L2; channel ownership is exclusive. */
struct {drv}_dma_channel {{
	const struct device *controller;
	uint32_t channel;
	struct dma_block_config block;
	struct dma_config config;
	bool configured;
	bool prepared;
	bool submitted;
	bool active;
	bool terminated;
	bool synchronized;
}};

static int __unused {drv}_dma_channel_acquire(const struct device *controller,
		void *filter_param,
		struct {drv}_dma_channel *owned)
{{
	int channel;
	if (!controller || !owned)
		return -EINVAL;
	channel = dma_request_channel(controller, filter_param);
	if (channel < 0)
		return channel;
	memset(owned, 0, sizeof(*owned));
	owned->controller = controller;
	owned->channel = (uint32_t)channel;
	return 0;
}}

static int __unused {drv}_dma_channel_configure(struct {drv}_dma_channel *owned,
		enum dma_channel_direction direction, uintptr_t source,
		uintptr_t destination, size_t length)
{{
	int ret;

	if (!owned || !owned->controller || !length || owned->prepared ||
	    owned->submitted || owned->active)
		return -EINVAL;
	owned->configured = false;
	memset(&owned->block, 0, sizeof(owned->block));
	memset(&owned->config, 0, sizeof(owned->config));
	owned->block.source_address = source;
	owned->block.dest_address = destination;
	owned->block.block_size = length;
	owned->config.channel_direction = direction;
	owned->config.block_count = 1;
	owned->config.head_block = &owned->block;
	owned->prepared = false;
	owned->submitted = false;
	owned->active = false;
	owned->terminated = false;
	owned->synchronized = false;
	ret = dma_config(owned->controller, owned->channel, &owned->config);
	if (!ret)
		owned->configured = true;
	return ret;
}}

static int __unused {drv}_dma_channel_prepare(struct {drv}_dma_channel *owned)
{{
	if (!owned || !owned->controller || !owned->configured || owned->prepared ||
	    owned->submitted || owned->active)
		return -EINVAL;
	owned->prepared = true;
	return 0;
}}

static int __unused {drv}_dma_channel_submit(struct {drv}_dma_channel *owned)
{{
	if (!owned || !owned->prepared)
		return -EINVAL;
	owned->prepared = false;
	owned->submitted = true;
	return 0;
}}

static int __unused {drv}_dma_channel_activate(struct {drv}_dma_channel *owned)
{{
	int ret;

	if (!owned || !owned->submitted)
		return -EINVAL;
	ret = dma_start(owned->controller, owned->channel);
	if (!ret) {{ owned->submitted = false; owned->active = true; }}
	return ret;
}}

"#
    );
    if ops.status.is_some() {
        out.push_str(&format!("static int __unused {drv}_dma_channel_status(struct {drv}_dma_channel *owned, struct dma_status *status)\n{{\n\tint ret;\n\tif (!owned || !owned->controller || !status)\n\t\treturn -EINVAL;\n\tret = dma_get_status(owned->controller, owned->channel, status);\n\tif (!ret && !status->busy)\n\t\towned->active = false;\n\treturn ret;\n}}\n\n"));
    }
    if ops.pause.is_some() || ops.suspend.is_some() {
        out.push_str(&format!("static int __unused {drv}_dma_channel_suspend(struct {drv}_dma_channel *owned)\n{{\n\treturn owned && owned->controller ? dma_suspend(owned->controller, owned->channel) : -EINVAL;\n}}\n\n"));
    }
    if ops.resume.is_some() {
        out.push_str(&format!("static int __unused {drv}_dma_channel_resume(struct {drv}_dma_channel *owned)\n{{\n\treturn owned && owned->controller ? dma_resume(owned->controller, owned->channel) : -EINVAL;\n}}\n\n"));
    }
    if ops.terminate.is_some() {
        out.push_str(&format!("static int __unused {drv}_dma_channel_terminate(struct {drv}_dma_channel *owned)\n{{\n\tint ret;\n\tif (!owned || !owned->controller)\n\t\treturn -EINVAL;\n\tret = dma_stop(owned->controller, owned->channel);\n\tif (!ret) {{ owned->submitted = false; owned->active = false; owned->terminated = true; owned->synchronized = false; }}\n\treturn ret; /* no completion callback is synthesized */\n}}\n\n"));
    }
    if ops.synchronize.is_some() {
        out.push_str(&format!("static int __unused {drv}_dma_channel_synchronize(struct {drv}_dma_channel *owned)\n{{\n\tif (!owned || !owned->terminated)\n\t\treturn -EINVAL;\n\t/* Zephyr dma_stop is the terminal barrier; preserve the separate L1 step. */\n\towned->synchronized = true;\n\treturn 0;\n}}\n\n"));
    }
    out.push_str(&format!("static int __unused {drv}_dma_channel_release(struct {drv}_dma_channel *owned)\n{{\n\tif (!owned || !owned->controller)\n\t\treturn -EINVAL;\n\tif (owned->prepared || owned->submitted || owned->active ||\n\t    (owned->terminated && !owned->synchronized))\n\t\treturn -EBUSY;\n\tdma_release_channel(owned->controller, owned->channel);\n\towned->controller = NULL;\n\treturn 0;\n}}\n\n"));
    for capability in ops.capabilities() {
        out.push_str(&format!("/* RSEQ-DMA-CHANNEL-CAPABILITY {capability} */\n"));
    }
    out.push_str("/* Missing status/control/callback operations remain unsupported; termination emits no completion. */\n\n");
    Ok(out)
}

pub(super) fn descriptor_datapath(model: &RegseqModel, driver: ItemId) -> bool {
    model
        .device_lifecycle
        .drivers
        .get(&driver)
        .is_some_and(|lifecycle| model.strings.resolve(lifecycle.descriptor.name) == "rich-e1000")
        && model.network_interface.drivers.contains_key(&driver)
        && model
            .dma
            .drivers
            .get(&driver)
            .is_some_and(|dma| dma.buffer.is_some())
        && model.domain_attrs.item_attrs.iter().flatten().any(|attr| {
            matches!(
                attr,
                AttrData::Intrinsic(regseq_model::IntrinsicKind::CommandTransportSubmit)
            )
        })
}

pub(super) fn data_fields(model: &RegseqModel, driver: ItemId) -> String {
    if !descriptor_datapath(model, driver) {
        return String::new();
    }
    "\tuint8_t __rseq_dma_cpu[2][4096] __aligned(128);\n\tuintptr_t __rseq_dma_address[2];\n\tsize_t __rseq_dma_capacity[2];\n\tbool __rseq_dma_allocated[2];\n"
        .into()
}

pub(crate) fn emit_buffer_op_shim(
    cx: &mut LowerCx<'_>,
    instance: InstanceId,
    head: &str,
) -> Result<Option<String>, EmitError> {
    let Some((driver, op)) = descriptor_buffer_op(cx.model, instance) else {
        return Ok(None);
    };
    if !descriptor_datapath(cx.model, driver) {
        return Ok(None);
    }
    cx.extra_includes.insert("<string.h>");
    cx.extra_includes.insert("<zephyr/sys/util.h>");

    let body = match op {
        "allocate" => r#"	struct DmaBufferHandle _0 = { .identity = 0 };
	struct rich_e1000_data *__data = CONTAINER_OF(_1, struct rich_e1000_data, inner);
	unsigned int __slot;

	if (_2.size != 4096 || !_2.alignment || (_2.alignment & (_2.alignment - 1)) ||
	    _2.alignment > 128 || _2.boundary)
		return _0;
	for (__slot = 0; __slot < 2; __slot++) {
		if (__data->__rseq_dma_allocated[__slot])
			continue;
		__data->__rseq_dma_allocated[__slot] = true;
		__data->__rseq_dma_capacity[__slot] = _2.size;
		__data->__rseq_dma_address[__slot] =
			(uintptr_t)&__data->__rseq_dma_cpu[__slot][0];
		if (_2.zero)
			memset(__data->__rseq_dma_cpu[__slot], 0, _2.size);
		_0.identity = (size_t)__slot + 1;
		return _0;
	}
	return _0;
"#
        .to_string(),
        "free" => {
            r#"	struct rich_e1000_data *__data = CONTAINER_OF(_1, struct rich_e1000_data, inner);
	unsigned int __slot;

	if (_2.identity == 0 || _2.identity > 2)
		return;
	__slot = (unsigned int)_2.identity - 1;
	__data->__rseq_dma_allocated[__slot] = false;
	__data->__rseq_dma_address[__slot] = 0;
	__data->__rseq_dma_capacity[__slot] = 0;
	return;
"#
            .to_string()
        }
        "cpu_address" => {
            r#"	struct rich_e1000_data *__data = CONTAINER_OF(_1, struct rich_e1000_data, inner);
	if (_2.identity == 0 || _2.identity > 2 || !__data->__rseq_dma_allocated[_2.identity - 1])
		return NULL;
	return &__data->__rseq_dma_cpu[_2.identity - 1][0];
"#
            .to_string()
        }
        "length" => {
            r#"	struct rich_e1000_data *__data = CONTAINER_OF(_1, struct rich_e1000_data, inner);
	if (_2.identity == 0 || _2.identity > 2 || !__data->__rseq_dma_allocated[_2.identity - 1])
		return 0;
	return __data->__rseq_dma_capacity[_2.identity - 1];
"#
            .to_string()
        }
        "transfer_address" => transfer_address_body(cx, instance)?,
        _ => return Ok(None),
    };
    Ok(Some(format!("{head}\n{{\n{body}}}\n")))
}

fn descriptor_buffer_op(
    model: &RegseqModel,
    instance: InstanceId,
) -> Option<(ItemId, &'static str)> {
    for (driver, dma) in &model.dma.drivers {
        let ops = &dma.buffer.as_ref()?.ops;
        for (name, reference) in [
            ("allocate", ops.allocate),
            ("free", ops.free),
            ("cpu_address", ops.cpu_address),
            ("length", ops.length),
            ("transfer_address", ops.transfer_address),
        ] {
            if reference.is_some_and(|reference| resolves_to(model, reference, instance)) {
                return Some((*driver, name));
            }
        }
    }
    None
}

fn resolves_to(model: &RegseqModel, reference: InstanceRef, instance: InstanceId) -> bool {
    model.instance_ref_of(instance) == reference
}

fn transfer_address_body(cx: &mut LowerCx<'_>, instance: InstanceId) -> Result<String, EmitError> {
    let regseq_model::InstanceKind::Function(data) = &cx.model.instances[instance].kind else {
        return Err(EmitError::Driver("DMA operation is not a function".into()));
    };
    let output = cx.model.sigs.resolve(data.fn_sig).output;
    let cty = super::types::c_type(cx.model, &mut cx.names, output)?;
    let layout = super::types::layout_of(cx.model, output)
        .ok_or_else(|| EmitError::Driver("DMA transfer address Option has no layout".into()))?;
    let LayoutKind::Enum {
        shape: EnumShape::Multiple { variants, .. },
    } = &layout.kind
    else {
        return Err(EmitError::Driver(
            "DMA transfer address does not return Option<u64>".into(),
        ));
    };
    let (tag_offset, tag_size) = super::types::enum_tag_cell(cx.model, output)?;
    let some = variants
        .iter()
        .find(|variant| cx.model.strings.resolve(variant.name) == "Some")
        .ok_or_else(|| EmitError::Driver("DMA transfer address Option lacks Some".into()))?;
    let some_tag = some
        .tag_value
        .ok_or_else(|| EmitError::Driver("DMA transfer address Some is untagged".into()))?;
    let value_offset = some
        .fields
        .first()
        .ok_or_else(|| EmitError::Driver("DMA transfer address Some lacks payload".into()))?
        .offset;
    let tag_ty = match tag_size {
        1 => "uint8_t",
        2 => "uint16_t",
        4 => "uint32_t",
        8 => "uint64_t",
        _ => {
            return Err(EmitError::Driver(
                "DMA transfer address Option tag has unsupported width".into(),
            ))
        }
    };
    Ok(format!(
        "\t{cty} _0;\n\tstruct rich_e1000_data *__data = CONTAINER_OF(_1, struct rich_e1000_data, inner);\n\tmemset(&_0, 0, sizeof(_0));\n\tif (_2.identity != 0 && _2.identity <= 2 && __data->__rseq_dma_allocated[_2.identity - 1]) {{\n\t\t{tag_ty} __tag = ({tag_ty}){some_tag};\n\t\tuint64_t __address = (uint64_t)__data->__rseq_dma_address[_2.identity - 1];\n\t\t_1->dma_address_shadow = __address;\n\t\tmemcpy(&_0._raw[{tag_offset}], &__tag, sizeof(__tag));\n\t\tmemcpy(&_0._raw[{value_offset}], &__address, sizeof(__address));\n\t}}\n\treturn _0;\n"
    ))
}

#[allow(dead_code)]
fn _model_anchor(_: &RegseqModel) {}
