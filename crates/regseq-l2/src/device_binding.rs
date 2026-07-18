//! Device-lifecycle platform-instance binding identity.
//!
//! The Rust match descriptor identifies the reusable device. The platform
//! sidecar may additionally identify the concrete integration instance through
//! `[binding.amba]` and/or `[binding.devicetree]`. This binder preserves that
//! distinction in the model so each backend can select only the form consumed
//! by its ratified registration path.

use regseq_config::PlatformBindings;
use regseq_model::{AmbaPlatformBinding, PlatformBindingIdentity, RegseqModel};

/// Join the platform-instance identity into the model without changing the
/// authored device match set.
pub fn join_platform_binding_identity(model: &mut RegseqModel, bindings: &PlatformBindings) {
    let amba = bindings.device.amba.map(|binding| AmbaPlatformBinding {
        periphid: binding.periphid,
        mask: binding.mask,
    });
    let devicetree_compatible = bindings
        .device
        .devicetree
        .as_ref()
        .map(|binding| model.strings.intern(&binding.compatible));
    model.device_lifecycle.platform_binding = PlatformBindingIdentity {
        amba,
        devicetree_compatible,
    };
}

/// Device-lifecycle's L2 instance-binding tenant. The join is structural and
/// infallible; backend registration-form checks remain at emission.
pub struct DeviceBindingBinder;

impl crate::L2Binder for DeviceBindingBinder {
    fn name(&self) -> &'static str {
        "device-binding"
    }

    fn bind(
        &self,
        model: &mut RegseqModel,
        bindings: &PlatformBindings,
    ) -> Result<(), crate::L2Error> {
        join_platform_binding_identity(model, bindings);
        Ok(())
    }

    fn validate(&self, _model: &RegseqModel) -> Result<(), crate::L2Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_config::{
        AmbaBindingSpec, DeviceBindingSpec, DevicetreeBindingSpec, PlatformBindings,
    };
    use regseq_model::StableCrateId;

    #[test]
    fn joins_both_forms_without_an_authored_driver() {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let bindings = PlatformBindings {
            device: DeviceBindingSpec {
                amba: Some(AmbaBindingSpec {
                    periphid: 0x00ab_c123,
                    mask: 0x00ff_ffff,
                }),
                devicetree: Some(DevicetreeBindingSpec {
                    compatible: "regseq,serialpl011".into(),
                }),
            },
            ..PlatformBindings::default()
        };

        crate::bind_all(&mut model, &bindings).expect("bind_all");
        assert_eq!(
            model.device_lifecycle.platform_binding.amba,
            Some(AmbaPlatformBinding {
                periphid: 0x00ab_c123,
                mask: 0x00ff_ffff,
            })
        );
        let compatible = model
            .device_lifecycle
            .platform_binding
            .devicetree_compatible
            .expect("DT binding");
        assert_eq!(model.strings.resolve(compatible), "regseq,serialpl011");
    }

    #[test]
    fn absent_binding_clears_a_previous_join() {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        model.device_lifecycle.platform_binding.amba = Some(AmbaPlatformBinding {
            periphid: 1,
            mask: u32::MAX,
        });
        crate::bind_all(&mut model, &PlatformBindings::default()).expect("bind_all");
        assert!(model.device_lifecycle.platform_binding.is_empty());
    }
}
