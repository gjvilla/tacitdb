use crate::id::EntityId;
use serde::Serialize;

/// A stable identity anchor (design/001 §1.1). Entities carry no envelope and
/// no truth: what an entity *is* lives in claims about it. Never deleted.
#[derive(Debug, Clone, Serialize)]
pub struct Entity {
    id: EntityId,
    kind: String,
    label: String,
}

impl Entity {
    pub(crate) fn new(id: EntityId, kind: String, label: String) -> Self {
        Self { id, kind, label }
    }

    pub fn id(&self) -> EntityId {
        self.id
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}
