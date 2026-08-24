use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use ulid::Ulid;

/// A textual id did not name a record or entity. Parsing an id lets a caller
/// *name* something the ledger already minted; it never lets one be forged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a valid {1} id")]
pub struct IdParseError(String, &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecordId(Ulid);

impl RecordId {
    // Only the ledger mints ids (invariant 3's sibling: identity is engine-assigned).
    pub(crate) fn mint() -> Self {
        Self(Ulid::generate())
    }
}

impl fmt::Display for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rec_{}", self.0)
    }
}

impl FromStr for RecordId {
    type Err = IdParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let fail = || IdParseError(text.to_string(), "record");
        let body = text.strip_prefix("rec_").ok_or_else(fail)?;
        Ulid::from_string(body).map(Self).map_err(|_| fail())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(Ulid);

impl EntityId {
    pub(crate) fn mint() -> Self {
        Self(Ulid::generate())
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ent_{}", self.0)
    }
}

impl FromStr for EntityId {
    type Err = IdParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let fail = || IdParseError(text.to_string(), "entity");
        let body = text.strip_prefix("ent_").ok_or_else(fail)?;
        Ulid::from_string(body).map(Self).map_err(|_| fail())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_through_text() {
        let record = RecordId::mint();
        let entity = EntityId::mint();
        assert_eq!(record.to_string().parse::<RecordId>().unwrap(), record);
        assert_eq!(entity.to_string().parse::<EntityId>().unwrap(), entity);
    }

    #[test]
    fn the_prefix_keeps_the_two_id_spaces_apart() {
        let record = RecordId::mint();
        assert!(record.to_string().parse::<EntityId>().is_err());
        assert!("rec_not-a-ulid".parse::<RecordId>().is_err());
        assert!("".parse::<RecordId>().is_err());
    }
}
