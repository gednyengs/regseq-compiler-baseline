use std::path::Path;

#[test]
fn locked_transport_stage_accounts_for_all_frozen_gap_rows() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../experiments/os-api-gap-closure/coverage/C3-DCT.json");
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(value["schema"], "gap-closure-stage-coverage/1");
    assert_eq!(value["stage"], "C3-DCT");
    let rows = value["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 6);
    assert_eq!(
        rows.iter()
            .map(|row| row["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        (1..=6)
            .map(|index| format!("G-device-command-transport-{index:02}"))
            .collect::<Vec<_>>()
    );
    assert!(rows
        .iter()
        .all(|row| !row["check"].as_str().unwrap().is_empty()));
}
