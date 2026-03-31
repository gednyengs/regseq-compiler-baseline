#![feature(rustc_private)]

#[path = "../src/extract/callback_context.rs"]
mod callback_context;

use callback_context::CallbackContext;

#[test]
fn round_trip_through_short_labels() {
    for ctx in [
        CallbackContext::FrameworkManaged,
        CallbackContext::SubsystemSerialized,
        CallbackContext::HandlerManaged,
    ] {
        let label = ctx.short_label();
        let recovered = CallbackContext::from_short_label(label)
            .unwrap_or_else(|| panic!("failed to round-trip label '{label}'"));
        assert_eq!(recovered, ctx);
    }
}

#[test]
fn short_labels_are_accepted_values() {
    assert_eq!(CallbackContext::FrameworkManaged.short_label(), "F");
    assert_eq!(CallbackContext::SubsystemSerialized.short_label(), "S");
    assert_eq!(CallbackContext::HandlerManaged.short_label(), "H");
}

#[test]
fn display_names_are_deterministic() {
    assert_eq!(
        CallbackContext::FrameworkManaged.display_name(),
        "framework-managed"
    );
    assert_eq!(
        CallbackContext::SubsystemSerialized.display_name(),
        "subsystem-serialized"
    );
    assert_eq!(
        CallbackContext::HandlerManaged.display_name(),
        "handler-managed"
    );
}

#[test]
fn from_short_label_rejects_unknown() {
    assert_eq!(CallbackContext::from_short_label("X"), None);
    assert_eq!(CallbackContext::from_short_label("f"), None);
    assert_eq!(CallbackContext::from_short_label(""), None);
}
