// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared test data: fixed IDs for the golden records and a template filler.
//!
//! The IDs are arbitrary fixed UUIDv5-shaped values (golden IDs are UUIDv5 values, doc 20 §3.2); the ones
//! of the doc 05 §3.5 examples are reused where a record appears there.

#![allow(dead_code)]

pub mod lines;

/// `(token, UUID)`: `@TOKEN` in a template becomes the UUID.
pub const IDS: &[(&str, &str)] = &[
    ("PRJ", "3f2b8c1e-6d4a-5e07-9b1c-2a7d4e9f0c13"),
    ("LIN", "6d0a3f85-1b9c-5e27-8a4f-c2e7d9b0a613"),
    ("LOC_JE2416", "a41f6c2d-8e05-5d97-b3c1-5f2e8a7d9c60"),
    ("LOC_JE206", "2b6e4f81-9c03-5d7a-8e45-1f0a3c6d9b72"),
    ("FN_P01", "7d2c95e0-1f4a-5b38-a6e7-0c93b5d1f284"),
    ("SEC1", "0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e"),
    ("SEC9", "5c3a9e17-2d84-5b6f-a0c1-7e9d4b2f8a65"),
    ("EXT_L", "6b1e0d52-3f47-5a8c-9e21-c4d7f80a5b13"),
    ("P1", "7f3d9a20-5c81-5e64-b2a9-0d6e3f1c8b47"),
    ("P101", "8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a"),
    ("P102", "1a7c3e95-4b20-5d8f-9c61-e3f0a2b7d548"),
    ("P103", "9d2f7b40-6e15-5a83-b7c2-4a8e1d0f6c39"),
    ("P201", "4e8a1c63-7f29-5b04-8d5e-2c9b6f3a0e71"),
    ("P902", "c5b0e7a2-3d48-5f16-9a7b-8e1c4d2f0b96"),
    ("DEV_G1", "c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2"),
    ("DEV_X1031", "58e2b1d0-4c97-5a3f-8e16-d09a72c5e3f1"),
    ("DEV_X1031_1", "f19c3a7e-6b02-5e48-9d5a-c7e1340b8f2a"),
    ("DEV_X1032_1", "3c8e2f71-0a95-5d46-b1e8-6f2d9c4a7b0e"),
    ("DEV_S51", "0b9d4e62-8f13-5c75-a2d0-3e7f1b6c9a84"),
    ("DEV_W106_1", "6e2a8c14-9b37-5f05-8d6c-1a4e7b0f3d29"),
    ("DEV_W3_1", "2d7f0b85-4e6a-5c19-9f3b-8c1e5a2d7f60"),
    ("FN_G1", "1d6f3a92-7c05-5b8e-94a1-e3c2087f5d19"),
    ("FN_S51", "d40b6e93-25f7-5c1a-9e8d-7a3f1c05b6e2"),
    ("FN_F1", "3e7a1c58-90d4-5b26-8f3a-6c0d2e9b417e"),
    ("FN_F2_102", "8a5c2e07-1d93-5f48-b6e0-4c7a9d3f2b15"),
    ("FN_IP24", "b7e04a39-6c12-5d8e-a5f7-2e9c1b4d8063"),
    ("FN_X1031", "e6c19d47-3a85-5b20-9e64-7d0f2a8c5b13"),
    ("FN_X1031_1", "4a0f7c96-2e5b-5d31-8c79-b1e6d3a0f427"),
    ("FN_X1031_2", "9f3b6d02-7a4e-5c88-b215-0e8d4c7a1f96"),
    ("FN_X1032_2", "5d81e2b6-0c7f-5a49-9d3e-a6f2b8c1e074"),
    ("REP_F1", "6f03d8a5-c912-5e7b-8a40-1d5b39e2f7c6"),
    ("REP_X1031A", "0c4e9b71-5d28-5f3a-b6e1-9a7d2c0f4e85"),
    ("CON_W7", "b29e4f17-8d63-5a0c-9f71-3c6a2e8d05b1"),
    ("CON_W20", "5a8f1e3c-0d72-5b49-9e16-c3a7f0b8d254"),
    ("CON_W29", "7c1d5a83-2f9e-5b64-a0d8-e5b3f17c9a26"),
    ("CON_BR1", "a8e3c0f5-6b17-5d92-8e4a-1c7f9d2b5e30"),
    ("CON_W39", "2f6b9e14-8c03-5a7d-b9e5-4d1a0c8f7b62"),
    ("CON_W27", "e1a7d4c0-3b96-5f2e-8a51-7c0d9e4b2f83"),
    ("CON_W22", "93b5e0a8-4c17-5f6d-8b2e-a9d0c3f71e45"),
    ("CON_W27B", "0a6d3f19-e8c2-5b74-9f05-1d8b6e2a7c30"),
    ("CON_DP", "4d9c2a67-1e0b-5c83-9f46-b8a3e5d0c172"),
    ("JN_J5", "d3f8b1a4-7e62-5c09-a4d7-2b9e6f1c0a58"),
    ("JN_J1", "81c6e3d9-4a07-5b5f-9e28-c0f4a7d2b691"),
    ("COND_BN", "6a4d0e83-9f21-5c7b-8e06-d2b5c9a1f374"),
    ("COND_SH", "0f5c8a26-d3e9-5b41-a7c0-9e2d4b6f8a13"),
    ("POT_LS", "3b2e7f90-5a14-5d68-9c3f-e7a0d1b4c825"),
    ("POT_24", "c9d1a6e3-8b40-5f27-b5e8-0a3c6d9f2e71"),
    ("BOND_B1", "7e5a3c01-2d8f-5b96-a1e4-6f9c0b3d7a52"),
    ("RB_TOC", "2a9e6c14-7b05-5d83-8f62-c4e1a0d9b736"),
    ("RB_903", "f4b8d2e6-1c39-5a70-9d15-e8a3c7f0b264"),
    ("RB_PD201", "9c0e5a72-4f18-5b3d-a6c9-1d7e2b8f4a05"),
    ("FORM_TOC", "5e7c1b38-0a9d-5f62-8b4e-3c1a6d9e0f27"),
    ("FORM_FRAME", "1f8d4a6b-2c70-5e93-a5b1-0e7c3d9f6a84"),
    ("FORM_COVER", "8b3f0d59-6e21-5c4a-9b07-f2a8c5e1d3b6"),
    ("SYM_MCB", "2c7e9b40-5a18-5f63-9d2c-b8a1e07f4396"),
    ("FNDEF_BB", "5b9e2d70-3c41-5f8a-b6d2-19e4c7a03f58"),
    ("FNDEF_IP", "0d4a8e2f-6b13-5c97-a8e0-4f2c9b6d1a35"),
    ("FNDEF_MCB", "6c2f9a17-0e84-5b3d-9a6c-e1d7b4f0a289"),
    ("PART_PSU", "e5d2a8f1-0c37-5b94-8e6a-4f1d93c7b025"),
    ("PART_TWIN", "a07c4e19-d25b-5f83-9c16-4e8b0d7a2f35"),
    ("PART_TWINPE", "c18f5b03-7e4a-5d26-a9c1-3b8e0d6f2a74"),
    ("PART_533356", "b4e81f2c-7a06-5d95-8c3b-0f9e6a2d4c17"),
    ("PART_NEBU", "3d6a0c95-e2b8-5f41-9d07-a5c3e8f1b624"),
    ("LIB", "6a1d0f3e-8b25-5c74-9e1a-d3f60b8c2e47"),
    ("ITEM_PSU", "8b42c7d9-1e06-5a3f-b8d5-6c0e2f9a4b71"),
    ("ITEM_MFR", "0f7b2c94-5d3a-5e81-a6f9-2b8c0e4d7a13"),
    ("ITEM_SYM", "4c9e1a73-8d2f-5b06-9e5c-a1f7d3b0e862"),
    ("ITEM_FN", "7a3e0c58-2b91-5d4f-8c6a-e0b9f2d5a147"),
    ("ITEM_FRAME", "9e1b5d04-6c83-5a2f-b7e9-3d0a8c6f1b25"),
];

/// A fixed SHA-256 for library-copy fixtures.
pub const HASH: &str = "sha256:9f2c9f2c9f2c9f2c9f2c9f2c9f2c9f2c9f2c9f2c9f2c9f2c9f2c9f2c9f2c9f2c";

/// Replaces every `@TOKEN` of [`IDS`] (longest token first) and `@HASH`.
pub fn fill(template: &str) -> String {
    let mut ids: Vec<&(&str, &str)> = IDS.iter().collect();
    ids.sort_by_key(|(token, _)| std::cmp::Reverse(token.len()));
    let mut out = template.replace("@HASH", HASH);
    for (token, id) in ids {
        out = out.replace(&format!("@{token}"), id);
    }
    assert!(!out.contains('@'), "unfilled token in {out}");
    out
}
