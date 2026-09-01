use crate::id::EntityId;
use serde::Serialize;

/// A stable identity anchor (design/001 §1.1). Entities carry no envelope and
/// no truth: what an entity *is* lives in claims about it. Never deleted.
#[derive(Debug, Clone, Serialize)]
pub struct Entity {
    id: EntityId,
    kind: String,
    label: String,
    redacted: Option<crate::envelope::RedactionMark>,
}

impl Entity {
    pub(crate) fn new(id: EntityId, kind: String, label: String) -> Self {
        Self { id, kind, label, redacted: None }
    }

    pub(crate) fn husk(
        id: EntityId,
        kind: String,
        label: String,
        redacted: Option<crate::envelope::RedactionMark>,
    ) -> Self {
        Self { id, kind, label, redacted }
    }

    /// The receipt, when this entity's label was withheld by a rewrite
    /// (U-46, D-0053). `None` is the ordinary case.
    pub fn redacted(&self) -> Option<&crate::envelope::RedactionMark> {
        self.redacted.as_ref()
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
