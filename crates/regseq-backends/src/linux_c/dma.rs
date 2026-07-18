//! Locked DMA resources lowered to Linux DMA-API helpers.
//!
//! Buffer allocation/free remains explicit and paired. Channel endpoint names
//! are parameters supplied by L2; this module never invents one. Descriptor
//! ownership passes to the engine at submit, and asynchronous termination is
//! followed by the separate synchronize barrier before release.

use regseq_model::{
    AttrData, DmaBufferOps, DriverDma, EnumShape, InstanceId, InstanceRef, ItemId, LayoutKind,
    RegseqModel,
};

use super::body::LowerCx;

pub(super) fn emit(cx: &mut LowerCx<'_>, driver: ItemId, drv: &str) -> String {
    let Some(dma) = cx.model.dma.drivers.get(&driver) else {
        return String::new();
    };
    cx.extra_includes.insert("<linux/dma-mapping.h>");
    cx.extra_includes.insert("<linux/dmaengine.h>");
    cx.extra_includes.insert("<linux/errno.h>");
    cx.extra_includes.insert("<linux/err.h>");
    cx.extra_includes.insert("<linux/mm.h>");

    let mut out = String::new();
    if let Some(buffer) = &dma.buffer {
        if descriptor_datapath(cx.model, driver) {
            for capability in buffer.ops.capabilities() {
                out.push_str(&format!(
                    "/* RSEQ-DMA-BUFFER-CAPABILITY {capability} (composed descriptor ring) */\n"
                ));
            }
            out.push('\n');
        } else {
            out.push_str(&buffer_helpers(&buffer.ops, drv));
        }
    }
    if dma.channel.is_some() {
        out.push_str(&channel_helpers(dma, drv));
    }
    out
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

pub(super) fn container_fields(model: &RegseqModel, driver: ItemId) -> String {
    if !descriptor_datapath(model, driver) {
        return String::new();
    }
    "\tvoid *__rseq_dma_cpu[2];\n\tdma_addr_t __rseq_dma_address[2];\n\tsize_t __rseq_dma_capacity[2];\n"
        .into()
}

pub(crate) fn emit_buffer_op_shim(
    cx: &mut LowerCx<'_>,
    instance: InstanceId,
    head: &str,
) -> Result<Option<String>, super::EmitError> {
    let Some((driver, op)) = descriptor_buffer_op(cx.model, instance) else {
        return Ok(None);
    };
    if !descriptor_datapath(cx.model, driver) {
        return Ok(None);
    }
    cx.extra_includes.insert("<linux/container_of.h>");
    cx.extra_includes.insert("<linux/dma-mapping.h>");
    cx.extra_includes.insert("<linux/string.h>");

    let body = match op {
        "allocate" => r#"	struct DmaBufferHandle _0 = { .identity = 0 };
	struct rich_e1000_priv *__priv = container_of(_1, struct rich_e1000_priv, inner);
	unsigned int __slot;

	if (!_2.size || !_2.alignment || (_2.alignment & (_2.alignment - 1)) || _2.boundary)
		return _0;
	for (__slot = 0; __slot < 2; __slot++) {
		if (__priv->__rseq_dma_cpu[__slot])
			continue;
		__priv->__rseq_dma_cpu[__slot] = dma_alloc_coherent(&__priv->pdev->dev,
			_2.size, &__priv->__rseq_dma_address[__slot], GFP_KERNEL);
		if (!__priv->__rseq_dma_cpu[__slot])
			return _0;
		if ((__priv->__rseq_dma_address[__slot] & (_2.alignment - 1)) != 0) {
			dma_free_coherent(&__priv->pdev->dev, _2.size,
				__priv->__rseq_dma_cpu[__slot], __priv->__rseq_dma_address[__slot]);
			__priv->__rseq_dma_cpu[__slot] = NULL;
			__priv->__rseq_dma_address[__slot] = (dma_addr_t)0;
			return _0;
		}
		__priv->__rseq_dma_capacity[__slot] = _2.size;
		if (_2.zero)
			memset(__priv->__rseq_dma_cpu[__slot], 0, _2.size);
		_0.identity = (size_t)__slot + 1;
		return _0;
	}
	return _0;
"#
        .to_string(),
        "free" => {
            r#"	struct rich_e1000_priv *__priv = container_of(_1, struct rich_e1000_priv, inner);
	unsigned int __slot;

	if (_2.identity == 0 || _2.identity > 2)
		return;
	__slot = (unsigned int)_2.identity - 1;
	if (!__priv->__rseq_dma_cpu[__slot])
		return;
	dma_free_coherent(&__priv->pdev->dev, __priv->__rseq_dma_capacity[__slot],
		__priv->__rseq_dma_cpu[__slot], __priv->__rseq_dma_address[__slot]);
	__priv->__rseq_dma_cpu[__slot] = NULL;
	__priv->__rseq_dma_address[__slot] = (dma_addr_t)0;
	__priv->__rseq_dma_capacity[__slot] = 0;
	return;
"#
            .to_string()
        }
        "cpu_address" => {
            r#"	struct rich_e1000_priv *__priv = container_of(_1, struct rich_e1000_priv, inner);
	if (_2.identity == 0 || _2.identity > 2)
		return NULL;
	return __priv->__rseq_dma_cpu[_2.identity - 1];
"#
            .to_string()
        }
        "length" => {
            r#"	struct rich_e1000_priv *__priv = container_of(_1, struct rich_e1000_priv, inner);
	if (_2.identity == 0 || _2.identity > 2)
		return 0;
	return __priv->__rseq_dma_capacity[_2.identity - 1];
"#
            .to_string()
        }
        "transfer_address" => transfer_address_body(cx, instance, "rich_e1000_priv", false)?,
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

fn transfer_address_body(
    cx: &mut LowerCx<'_>,
    instance: InstanceId,
    container: &str,
    zephyr: bool,
) -> Result<String, super::EmitError> {
    let regseq_model::InstanceKind::Function(data) = &cx.model.instances[instance].kind else {
        return Err(super::EmitError::Driver(
            "DMA operation is not a function".into(),
        ));
    };
    let output = cx.model.sigs.resolve(data.fn_sig).output;
    let cty = super::types::c_type(cx.model, &mut cx.names, output)?;
    let layout = super::types::layout_of(cx.model, output).ok_or_else(|| {
        super::EmitError::Driver("DMA transfer address Option has no layout".into())
    })?;
    let LayoutKind::Enum {
        shape: EnumShape::Multiple { variants, .. },
    } = &layout.kind
    else {
        return Err(super::EmitError::Driver(
            "DMA transfer address does not return Option<u64>".into(),
        ));
    };
    let (tag_offset, tag_size) = super::types::enum_tag_cell(cx.model, output)?;
    let some = variants
        .iter()
        .find(|variant| cx.model.strings.resolve(variant.name) == "Some")
        .ok_or_else(|| super::EmitError::Driver("DMA transfer address Option lacks Some".into()))?;
    let some_tag = some
        .tag_value
        .ok_or_else(|| super::EmitError::Driver("DMA transfer address Some is untagged".into()))?;
    let value_offset = some
        .fields
        .first()
        .ok_or_else(|| super::EmitError::Driver("DMA transfer address Some lacks payload".into()))?
        .offset;
    let recover = if zephyr {
        format!("\tstruct {container} *__priv = CONTAINER_OF(_1, struct {container}, inner);\n")
    } else {
        format!("\tstruct {container} *__priv = container_of(_1, struct {container}, inner);\n")
    };
    let tag_ty = match tag_size {
        1 => "u8",
        2 => "u16",
        4 => "u32",
        8 => "u64",
        _ => {
            return Err(super::EmitError::Driver(
                "DMA transfer address Option tag has unsupported width".into(),
            ))
        }
    };
    Ok(format!(
        "\t{cty} _0;\n{recover}\tmemset(&_0, 0, sizeof(_0));\n\tif (_2.identity != 0 && _2.identity <= 2 && __priv->__rseq_dma_cpu[_2.identity - 1]) {{\n\t\t{tag_ty} __tag = ({tag_ty}){some_tag};\n\t\tu64 __address = (u64)__priv->__rseq_dma_address[_2.identity - 1];\n\t\t_1->dma_address_shadow = __address;\n\t\tmemcpy(&_0._raw[{tag_offset}], &__tag, sizeof(__tag));\n\t\tmemcpy(&_0._raw[{value_offset}], &__address, sizeof(__address));\n\t}}\n\treturn _0;\n"
    ))
}

fn buffer_helpers(ops: &DmaBufferOps, drv: &str) -> String {
    let mut out = format!(
        r#"/* RSEQ-DMA-BUFFER: explicit allocation identity and exactly-once release. */
struct {drv}_dma_buffer {{
	struct device *dev;
	void *cpu_address;
	dma_addr_t transfer_address;
	size_t capacity;
	size_t valid_length;
}};

static int __maybe_unused {drv}_dma_buffer_allocate(struct device *dev,
		size_t size, size_t alignment, size_t boundary, bool zero,
		struct {drv}_dma_buffer *buffer)
{{
	if (!dev || !buffer || !size || !alignment ||
	    (alignment & (alignment - 1)) || alignment > PAGE_SIZE || boundary)
		return -EINVAL;
	memset(buffer, 0, sizeof(*buffer));
	buffer->cpu_address = dma_alloc_coherent(dev, size,
		&buffer->transfer_address, GFP_KERNEL);
	if (!buffer->cpu_address)
		return -ENOMEM;
	buffer->dev = dev;
	buffer->capacity = size;
	buffer->valid_length = size;
	if (zero)
		memset(buffer->cpu_address, 0, size);
	return 0;
}}

static void __maybe_unused {drv}_dma_buffer_free(struct {drv}_dma_buffer *buffer)
{{
	if (!buffer || !buffer->cpu_address)
		return;
	dma_free_coherent(buffer->dev, buffer->capacity,
		buffer->cpu_address, buffer->transfer_address);
	buffer->cpu_address = NULL;
	buffer->transfer_address = (dma_addr_t)0;
	buffer->capacity = 0;
	buffer->valid_length = 0;
}}

"#
    );
    out.push_str(&format!(
        "static __maybe_unused void *{drv}_dma_buffer_cpu_address(const struct {drv}_dma_buffer *buffer)\n{{\n\treturn buffer ? buffer->cpu_address : NULL;\n}}\n\n\
         static size_t __maybe_unused {drv}_dma_buffer_length(const struct {drv}_dma_buffer *buffer)\n{{\n\treturn buffer ? buffer->valid_length : 0;\n}}\n\n"
    ));
    if ops.transfer_address.is_some() {
        out.push_str(&format!(
            "static int __maybe_unused {drv}_dma_buffer_transfer_address(const struct {drv}_dma_buffer *buffer, dma_addr_t *address)\n{{\n\tif (!buffer || !buffer->cpu_address || !address)\n\t\treturn -EINVAL;\n\t*address = buffer->transfer_address;\n\treturn 0;\n}}\n\n"
        ));
    }
    if ops.set_valid_length.is_some() {
        out.push_str(&format!(
            "static int __maybe_unused {drv}_dma_buffer_set_valid_length(struct {drv}_dma_buffer *buffer, size_t length)\n{{\n\tif (!buffer || !buffer->cpu_address || length > buffer->capacity)\n\t\treturn -EINVAL;\n\tbuffer->valid_length = length;\n\treturn 0;\n}}\n\n"
        ));
    }
    for capability in ops.capabilities() {
        out.push_str(&format!("/* RSEQ-DMA-BUFFER-CAPABILITY {capability} */\n"));
    }
    out.push_str("/* No public transfer-address or valid-length operation is emitted when the capability is absent. */\n\n");
    out
}

fn channel_helpers(dma: &DriverDma, drv: &str) -> String {
    let ops = &dma.channel.as_ref().expect("channel present").ops;
    let mut out = format!(
        r#"/* RSEQ-DMA-CHANNEL: endpoint resolution is L2 and passed as `endpoint`. */
struct {drv}_dma_channel {{
	struct dma_chan *channel;
	struct dma_async_tx_descriptor *prepared;
	dma_cookie_t cookie;
	enum dma_transfer_direction direction;
	dma_addr_t source;
	dma_addr_t destination;
	size_t length;
	bool configured;
	bool submitted;
	bool active;
	bool terminated;
	bool synchronized;
}};

static int __maybe_unused {drv}_dma_channel_acquire(struct device *dev,
		const char *endpoint, struct {drv}_dma_channel *owned)
{{
	if (!dev || !endpoint || !owned)
		return -EINVAL;
	memset(owned, 0, sizeof(*owned));
	owned->channel = dma_request_chan(dev, endpoint);
	if (IS_ERR(owned->channel)) {{
		int ret = PTR_ERR(owned->channel);
		owned->channel = NULL;
		return ret;
	}}
	return 0;
}}

static int __maybe_unused {drv}_dma_channel_configure(
		struct {drv}_dma_channel *owned, enum dma_transfer_direction direction,
		dma_addr_t source, dma_addr_t destination, size_t length)
{{
	struct dma_slave_config config = {{0}};

	if (!owned || !owned->channel || !length || owned->prepared ||
	    owned->submitted || owned->active)
		return -EINVAL;
	owned->configured = false;
	config.direction = direction;
	config.src_addr = source;
	config.dst_addr = destination;
	if (direction != DMA_MEM_TO_MEM) {{
		int ret = dmaengine_slave_config(owned->channel, &config);
		if (ret)
			return ret;
	}}
	owned->direction = direction;
	owned->source = source;
	owned->destination = destination;
	owned->length = length;
	owned->configured = true;
	owned->submitted = false;
	owned->active = false;
	owned->terminated = false;
	owned->synchronized = false;
	return 0;
}}

static int __maybe_unused {drv}_dma_channel_prepare(struct {drv}_dma_channel *owned)
{{
	dma_addr_t memory;

	if (!owned || !owned->channel || !owned->configured || owned->prepared ||
	    owned->submitted || owned->active)
		return -EINVAL;
	memory = owned->direction == DMA_DEV_TO_MEM
		? owned->destination : owned->source;
	owned->prepared = owned->direction == DMA_MEM_TO_MEM
		? dmaengine_prep_dma_memcpy(owned->channel, owned->destination,
			owned->source, owned->length,
			DMA_CTRL_ACK)
		: dmaengine_prep_slave_single(owned->channel, memory, owned->length,
			owned->direction, DMA_CTRL_ACK);
	return owned->prepared ? 0 : -EIO;
}}

static int __maybe_unused {drv}_dma_channel_submit(struct {drv}_dma_channel *owned)
{{
	int ret;

	if (!owned || !owned->prepared)
		return -EINVAL;
	owned->cookie = dmaengine_submit(owned->prepared);
	owned->prepared = NULL; /* ownership passed to the engine */
	ret = dma_submit_error(owned->cookie);
	if (ret)
		return ret;
	owned->submitted = true;
	return 0;
}}

static int __maybe_unused {drv}_dma_channel_activate(struct {drv}_dma_channel *owned)
{{
	if (!owned || !owned->channel || !owned->submitted ||
	    dma_submit_error(owned->cookie))
		return -EINVAL;
	dma_async_issue_pending(owned->channel);
	owned->submitted = false;
	owned->active = true;
	return 0;
}}

"#
    );
    if ops.status.is_some() {
        out.push_str(&format!(
            "static enum dma_status __maybe_unused {drv}_dma_channel_status(struct {drv}_dma_channel *owned, struct dma_tx_state *state)\n{{\n\tenum dma_status status;\n\n\tif (!owned || !owned->channel)\n\t\treturn DMA_ERROR;\n\tstatus = dmaengine_tx_status(owned->channel, owned->cookie, state);\n\tif (status == DMA_COMPLETE || status == DMA_ERROR)\n\t\towned->active = false;\n\treturn status;\n}}\n\n"
        ));
    }
    if ops.pause.is_some() {
        out.push_str(&format!("static int __maybe_unused {drv}_dma_channel_pause(struct {drv}_dma_channel *owned)\n{{\n\treturn owned && owned->channel ? dmaengine_pause(owned->channel) : -EINVAL;\n}}\n\n"));
    }
    if ops.resume.is_some() {
        out.push_str(&format!("static int __maybe_unused {drv}_dma_channel_resume(struct {drv}_dma_channel *owned)\n{{\n\treturn owned && owned->channel ? dmaengine_resume(owned->channel) : -EINVAL;\n}}\n\n"));
    }
    if ops.terminate.is_some() {
        out.push_str(&format!(
            "static int __maybe_unused {drv}_dma_channel_terminate(struct {drv}_dma_channel *owned)\n{{\n\tint ret;\n\tif (!owned || !owned->channel)\n\t\treturn -EINVAL;\n\tret = dmaengine_terminate_async(owned->channel);\n\tif (!ret) {{ owned->submitted = false; owned->active = false; owned->terminated = true; owned->synchronized = false; }}\n\treturn ret; /* no completion callback is synthesized */\n}}\n\n"
        ));
    }
    if ops.synchronize.is_some() {
        out.push_str(&format!(
            "static int __maybe_unused {drv}_dma_channel_synchronize(struct {drv}_dma_channel *owned)\n{{\n\tif (!owned || !owned->channel || !owned->terminated)\n\t\treturn -EINVAL;\n\tdmaengine_synchronize(owned->channel);\n\towned->synchronized = true;\n\treturn 0;\n}}\n\n"
        ));
    }
    out.push_str(&format!(
        "static int __maybe_unused {drv}_dma_channel_release(struct {drv}_dma_channel *owned)\n{{\n\tif (!owned || !owned->channel)\n\t\treturn -EINVAL;\n\tif (owned->prepared || owned->submitted || owned->active ||\n\t    (owned->terminated && !owned->synchronized))\n\t\treturn -EBUSY;\n\tdma_release_channel(owned->channel);\n\towned->channel = NULL;\n\treturn 0;\n}}\n\n"
    ));
    for capability in ops.capabilities() {
        out.push_str(&format!("/* RSEQ-DMA-CHANNEL-CAPABILITY {capability} */\n"));
    }
    out.push_str("/* Missing status/control/callback operations remain unsupported; termination emits no completion. */\n\n");
    out
}

#[allow(dead_code)]
fn _model_anchor(_: &RegseqModel) {}
