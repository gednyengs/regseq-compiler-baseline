//! Forward-declared domain attribute tables.

use serde::{Deserialize, Serialize};

/// Domain attribute sibling tables.
// FORWARD: P1a-DS-09
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DomainAttrTables {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_attr_tables_construct() {
        let tables = DomainAttrTables::default();
        assert_eq!(tables, DomainAttrTables {});
    }
}
