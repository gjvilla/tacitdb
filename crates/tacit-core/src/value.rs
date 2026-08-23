use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// A typed claim value. Deliberately closed: open-ended structure belongs in
/// `ClaimContent::Text` or the pattern shape, not in attribute values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Text(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Timestamp(Timestamp),
}
