# Codegen-coverage ledger

- **Verdict:** `not-pass`
- **Toolchain:** `cc` (cc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0)
- **Matrix rows:** 176
- **Covered rows:** 169 (145 with a green fixture)

## Drift: covered-without-green

- `AK-1`
- `AK-2`
- `AK-3`
- `AK-4`
- `AK-5`
- `AK-6`
- `AK-7`
- `AK-8`
- `BO-25`
- `CK-11`
- `CK-9`
- `CO-12`
- `DL-5`
- `FS-8`
- `FS-9`
- `IN-6`
- `IT-13`
- `IT-14`
- `PC-1`
- `PC-3`
- `TM-4`
- `TM-6`
- `TY-14`
- `TY-7`

## Rows

| id | status | result | fixtures |
| -- | ------ | ------ | -------- |
| AG-1 | Covered | Pass | b3_structs |
| AG-2 | Covered | Pass | b3_structs |
| AG-3 | Covered | Pass | b3_structs |
| AG-4 | Covered | Pass | b4_enums |
| AG-5 | Covered | Pass | b3_structs |
| AG-6 | Covered | Pass | b9_closures |
| AG-7 | Covered | Pass | b3_refs_slices |
| AK-1 | Covered | Fail | - |
| AK-2 | Covered | Fail | - |
| AK-3 | Covered | Fail | - |
| AK-4 | Covered | Fail | - |
| AK-5 | Covered | Fail | - |
| AK-6 | Covered | Fail | - |
| AK-7 | Covered | Fail | - |
| AK-8 | Covered | Fail | - |
| BD-1 | Covered | Pass | b1_int_scalars |
| BD-10 | Covered | Pass | b1_int_scalars |
| BD-2 | Covered | Pass | b1_int_scalars |
| BD-3 | Covered | Pass | b1_int_scalars |
| BD-4 | OutOfScope | NotApplicable | - |
| BD-5 | Covered | Pass | b1_int_scalars |
| BD-6 | OutOfScope | NotApplicable | - |
| BD-7 | Covered | Pass | b1_int_scalars |
| BD-8 | Covered | Pass | b1_int_scalars |
| BD-9 | Covered | Pass | b1_int_scalars |
| BO-1..5 | Covered | Pass | b1_int_scalars |
| BO-12..14 | Covered | Pass | b1_int_scalars |
| BO-15..18 | Covered | Pass | b1_int_scalars |
| BO-19..24 | Covered | Pass | b1_int_scalars |
| BO-25 | Covered | Fail | - |
| BO-26 | Covered | Pass | b3_refs_slices |
| BO-6..8 | Covered | Pass | b1_int_scalars |
| BO-9..11 | Covered | Pass | b1_int_scalars |
| CK-1 | Covered | Pass | b2_floats_casts |
| CK-10 | Covered | Pass | b2_floats_casts |
| CK-11 | Covered | Fail | - |
| CK-2 | Covered | Pass | b2_floats_casts |
| CK-3 | Covered | Pass | b5_control_flow |
| CK-4 | Covered | Pass | b2_floats_casts |
| CK-5 | Covered | Pass | b2_floats_casts |
| CK-6 | Covered | Pass | b2_floats_casts |
| CK-7 | Covered | Pass | b2_floats_casts |
| CK-8 | Covered | Pass | b2_floats_casts |
| CK-9 | Covered | Fail | - |
| CO-1 | Covered | Pass | b1_int_scalars |
| CO-10 | Covered | Pass | b3_refs_slices |
| CO-11 | Covered | Pass | b3_refs_slices |
| CO-12 | Covered | Fail | - |
| CO-13 | Covered | Pass | b3_refs_slices |
| CO-14 | Covered | Pass | b3_refs_slices |
| CO-2 | Covered | Pass | b1_int_scalars |
| CO-3 | Covered | Pass | b1_int_scalars |
| CO-4 | Covered | Pass | b1_int_scalars |
| CO-5 | Covered | Pass | b2_floats_casts |
| CO-6 | Covered | Pass | b3_structs |
| CO-7 | Covered | Pass | b3_structs |
| CO-8 | Covered | Pass | b3_structs |
| CO-9 | Covered | Pass | b4_enums |
| DL-1 | Covered | Pass | diag_levels |
| DL-2 | Covered | Pass | diag_empty, diag_levels, diag_newline |
| DL-3 | Covered | Pass | diag_bool, diag_escapes, diag_hex, diag_nul, diag_numeric_widths |
| DL-4 | Covered | Pass | diag_nul, diag_str_long, diag_str_nonstatic, diag_str_static |
| DL-5 | Covered | Fail | - |
| DL-6 | Covered | Pass | diag_levels |
| DL-7 | Covered | Pass | diag_levels |
| FS-1 | Covered | Pass | b1_int_scalars |
| FS-10 | Covered | Pass | b7_fn_sigs |
| FS-11 | OutOfScope | NotApplicable | - |
| FS-2 | Covered | Pass | b1_int_scalars |
| FS-3 | Covered | Pass | b1_int_scalars |
| FS-4 | Covered | Pass | b7_fn_sigs |
| FS-5 | Covered | Pass | b7_variadic |
| FS-6 | OutOfScope | NotApplicable | - |
| FS-7 | Covered | Pass | b7_fn_sigs |
| FS-8 | Covered | Fail | - |
| FS-9 | Covered | Fail | - |
| IN-1 | Covered | Pass | b1_int_scalars |
| IN-10 | Covered | Pass | b4_enums |
| IN-11 | Covered | Pass | b3_structs |
| IN-2 | Covered | Pass | b1_int_scalars |
| IN-3 | Covered | Pass | b8_item_kinds |
| IN-4 | Covered | Pass | b1_int_scalars |
| IN-5 | Covered | Pass | b5_control_flow |
| IN-5b | OutOfScope | NotApplicable | - |
| IN-6 | Covered | Fail | - |
| IN-7 | Covered | Pass | b8_item_kinds |
| IN-8 | Covered | Pass | b8_item_kinds |
| IN-9 | Covered | Pass | b3_structs |
| IT-1 | Covered | Pass | b1_int_scalars |
| IT-10 | OutOfScope | NotApplicable | - |
| IT-11 | Covered | Pass | b3_structs |
| IT-12 | Covered | Pass | b3_structs |
| IT-13 | Covered | Fail | - |
| IT-14 | Covered | Fail | - |
| IT-2 | Covered | Pass | b8_item_kinds |
| IT-3 | Covered | Pass | b8_item_kinds |
| IT-4 | Covered | Pass | b3_structs |
| IT-5 | Covered | Pass | b4_enums |
| IT-6 | Covered | Pass | b3_structs |
| IT-7 | Covered | Pass | b1_int_scalars |
| IT-8 | Covered | Pass | b8_item_kinds |
| IT-9 | Covered | Pass | b8_item_kinds |
| LY-1 | Covered | Pass | b3_structs |
| LY-10 | Covered | Pass | b4_enums |
| LY-11 | Covered | Pass | b4_enums |
| LY-12 | Covered | Pass | b4_enums |
| LY-13 | Covered | Pass | b4_enums |
| LY-14 | Covered | Pass | b4_enums |
| LY-2 | Covered | Pass | b3_structs |
| LY-3 | Covered | Pass | b3_structs |
| LY-4 | Covered | Pass | b3_structs |
| LY-5 | Covered | Pass | b4_enums |
| LY-6 | Covered | Pass | b3_structs |
| LY-7 | Covered | Pass | b4_enums |
| LY-8 | Covered | Pass | b4_enums |
| LY-9 | Covered | Pass | b4_enums |
| OP-1 | Covered | Pass | b1_int_scalars |
| OP-2 | Covered | Pass | b1_int_scalars |
| OP-3 | Covered | Pass | b1_int_scalars |
| PC-1 | Covered | Fail | - |
| PC-2 | Covered | Pass | b5_control_flow |
| PC-3 | Covered | Fail | - |
| PC-4 | Covered | Pass | b3_refs_slices |
| PL-1 | Covered | Pass | b3_structs |
| PL-2 | Covered | Pass | b3_refs_slices |
| PL-3 | Covered | Pass | b3_structs |
| PL-4 | Covered | Pass | b3_structs |
| PL-5 | Covered | Pass | b3_refs_slices |
| PL-6 | Covered | Pass | b3_refs_slices |
| PL-7 | Covered | Pass | b3_refs_slices |
| PL-8 | Covered | Pass | b4_enums |
| RV-1 | Covered | Pass | b1_int_scalars |
| RV-2 | Covered | Pass | b3_structs |
| RV-3 | Covered | Pass | b3_refs_slices |
| RV-4 | Covered | Pass | b3_refs_slices |
| RV-5 | Covered | Pass | b2_floats_casts |
| RV-6 | Covered | Pass | b1_int_scalars |
| RV-7 | Covered | Pass | b1_int_scalars |
| RV-8 | Covered | Pass | b4_enums |
| RV-9 | Covered | Pass | b3_structs |
| ST-1 | Covered | Pass | b1_int_scalars |
| ST-2 | Covered | Pass | b4_enums |
| ST-3 | Covered | Pass | b3_refs_slices |
| ST-4 | Covered | Pass | b3_refs_slices |
| TM-1 | Covered | Pass | b5_control_flow |
| TM-10 | Covered | Pass | b5_control_flow |
| TM-2 | Covered | Pass | diag_levels |
| TM-3 | Covered | Pass | diag_levels |
| TM-4 | Covered | Fail | - |
| TM-5 | Covered | Pass | diag_levels |
| TM-6 | Covered | Fail | - |
| TM-7 | Covered | Pass | b5_control_flow |
| TM-8 | Covered | Pass | diag_levels |
| TM-9 | Covered | Pass | b5_control_flow |
| TY-1 | Covered | Pass | b1_int_scalars |
| TY-10 | Covered | Pass | b3_refs_slices |
| TY-11 | Covered | Pass | b3_refs_slices |
| TY-12 | OutOfScope | NotApplicable | - |
| TY-13 | Covered | Pass | b5_control_flow |
| TY-14 | Covered | Fail | - |
| TY-15 | Covered | Pass | b9_closures |
| TY-16 | Covered | Pass | b3_structs |
| TY-17 | Covered | Pass | b3_structs |
| TY-18 | Covered | Pass | b4_enums |
| TY-19 | Covered | Pass | b3_structs |
| TY-2 | Covered | Pass | b1_int_scalars |
| TY-3 | Covered | Pass | b1_int_scalars |
| TY-4 | Covered | Pass | b1_int_scalars |
| TY-5 | Covered | Pass | b2_floats_casts |
| TY-6 | Covered | Pass | b3_refs_slices |
| TY-7 | Covered | Fail | - |
| TY-8 | Covered | Pass | b3_structs |
| TY-9 | Covered | Pass | b3_refs_slices |
| UN-1 | Covered | Pass | b1_int_scalars |
| UN-2 | Covered | Pass | b1_int_scalars |
| UN-3 | Covered | Pass | b3_refs_slices |
