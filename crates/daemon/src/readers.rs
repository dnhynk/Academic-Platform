//! Read-only connection creation for concurrent query clients.

use academic_store::{connection::ReaderConnection, error::StoreError, profile::SyntheticProfile};

/// Cloneable factory that can create only OS-read-only/query-only connections.
#[derive(Debug, Clone)]
pub struct ReaderFactory {
    profile: SyntheticProfile,
}

impl ReaderFactory {
    pub(crate) const fn new(profile: SyntheticProfile) -> Self {
        Self { profile }
    }

    /// Opens a new read-only/query-only connection.
    pub fn open(&self) -> Result<ReaderConnection, StoreError> {
        self.profile.open_reader()
    }
}
