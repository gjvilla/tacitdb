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

impl Value {
    /// The searchable rendering of a value. Numbers and timestamps are indexed
    /// as their text form so a query can name them.
    pub fn as_search_text(&self) -> String {
        match self {
            Value::Text(t) => t.clone(),
            Value::Number(n) => n.to_string(),
            Value::Integer(i) => i.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::Timestamp(t) => t.to_string(),
        }
    }
}
