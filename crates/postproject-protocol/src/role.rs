//! Local authority and passive materialization are distinct operating roles.

/// Persisted local role, independent of production/source-history identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreRole {
    /// Owns semantic writes, authority time and worker capabilities.
    Authority,
    /// Applies validated source facts only, without worker authority.
    PassiveMirror,
}
