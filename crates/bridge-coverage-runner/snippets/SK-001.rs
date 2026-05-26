#![no_std]
// SK-001: `SyntheticItemKind::None` (source-derived). Any plain source
// function is interned as an Item with no synthetic kind.
pub fn no_op() {}
