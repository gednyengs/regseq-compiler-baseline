//! Locked DMA resources lowered to Linux DMA-API helpers.
//!
//! Buffer allocation/free remains explicit and paired. Channel endpoint names
//! are parameters supplied by L2; this module never invents one. Descriptor
//! ownership passes to the engine at submit, and asynchronous termination is
//! followed by the separate synchronize barrier before release.

use regseq_model::{DmaBufferOps, DriverDma, ItemId, RegseqModel};

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
        out.push_str(&buffer_helpers(&buffer.ops, drv));
    }
    if dma.channel.is_some() {
        out.push_str(&channel_helpers(dma, drv));
    }
    out
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
