use postproject_protocol::RecordManifest;

/// A historical checkpoint never validates its snapshots against current facts.
#[derive(Clone, Copy)]
pub(super) enum Context<'a> {
    Prefix(&'a RecordManifest),
    Checkpoint(u64),
}

impl Context<'_> {
    pub(super) fn sequence(self) -> u64 {
        match self {
            Self::Prefix(manifest) => manifest.revision().sequence(),
            Self::Checkpoint(head) => head,
        }
    }

    pub(super) fn prefix(self) -> bool {
        matches!(self, Self::Prefix(_))
    }

    pub(super) fn at_boundary(self, boundary: Option<u64>) -> Self {
        match self {
            Self::Checkpoint(head) => Self::Checkpoint(boundary.unwrap_or(head)),
            prefix @ Self::Prefix(_) => prefix,
        }
    }
}
