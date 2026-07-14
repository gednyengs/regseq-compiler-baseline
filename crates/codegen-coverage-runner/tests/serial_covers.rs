use std::path::Path;

#[test]
fn locked_serial_stage_accounts_for_all_frozen_pl011_rows() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../experiments/os-api-gap-closure/coverage/C1-serial.json");
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).expect("read C1 serial coverage"))
            .expect("parse C1 serial coverage");
    assert_eq!(value["schema"], "gap-closure-stage-coverage/1");
    assert_eq!(value["stage"], "C1-serial");
    let rows = value["rows"].as_array().expect("rows array");
    assert_eq!(rows.len(), 21);
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row["id"], format!("PL011-S{:02}", index + 1));
        assert!(matches!(
            row["owner"].as_str(),
            Some("serial-port" | "backend" | "other-construct")
        ));
        assert!(!row["check"].as_str().unwrap_or_default().is_empty());
    }
    let owners = rows
        .iter()
        .map(|row| row["owner"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(owners.iter().filter(|owner| **owner == "serial-port").count(), 15);
    assert_eq!(owners.iter().filter(|owner| **owner == "backend").count(), 4);
    assert_eq!(owners.iter().filter(|owner| **owner == "other-construct").count(), 2);
}
