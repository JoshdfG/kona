//! RPC validator component used in interop.
//!
//! [`InteropTxValidator`] parses inbox entries from [`AccessListItem`]s, and queries a
//! superchain supervisor for their validity via RPC using the [`CheckAccessList`] API.

mod api;
pub use api::CheckAccessListClient;

mod error;
pub use error::InteropTxValidatorError;

use alloy_eips::eip2930::AccessListItem;
use alloy_primitives::B256;
use core::time::Duration;
use kona_interop::{ExecutingDescriptor, SafetyLevel, parse_access_list_items_to_inbox_entries};

/// Interacts with a Supervisor to validate inbox entries extracted from [`AccessListItem`]s.
#[async_trait::async_trait]
pub trait InteropTxValidator {
    /// The supervisor client type.
    type SupervisorClient: CheckAccessListClient + Send + Sync;

    /// Default duration that message validation is not allowed to exceed.
    ///
    /// Note: this has no effect unless shorter than timeout configured for
    /// [`Self::SupervisorClient`] type.
    const DEFAULT_TIMEOUT: Duration;

    /// Returns reference to supervisor client. Used in default trait method implementations.
    fn supervisor_client(&self) -> &Self::SupervisorClient;

    /// Extracts inbox entries from the [`AccessListItem`]s if there are any.
    fn parse_access_list(access_list_items: &[AccessListItem]) -> impl Iterator<Item = &B256> {
        parse_access_list_items_to_inbox_entries(access_list_items.iter())
    }

    /// Validates a list of inbox entries against a Supervisor.
    ///
    /// Times out RPC requests after given timeout, as long as this timeout is shorter
    /// than the underlying request timeout configured for [`Self::SupervisorClient`] type.
    async fn validate_messages_with_timeout(
        &self,
        inbox_entries: &[B256],
        safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
        timeout: Duration,
    ) -> Result<(), InteropTxValidatorError> {
        // Validate messages against supervisor with timeout.
        tokio::time::timeout(
            timeout,
            self.supervisor_client().check_access_list(inbox_entries, safety, executing_descriptor),
        )
        .await
        .map_err(|_| InteropTxValidatorError::ValidationTimeout(timeout.as_secs()))?
    }

    /// Validates a list of inbox entries against a Supervisor.
    ///
    /// Times out RPC requests after [`Self::DEFAULT_TIMEOUT`], as long as this timeout is shorter
    /// than the underlying request timeout configured for [`Self::SupervisorClient`] type.
    async fn validate_messages(
        &self,
        inbox_entries: &[B256],
        safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), InteropTxValidatorError> {
        self.validate_messages_with_timeout(
            inbox_entries,
            safety,
            executing_descriptor,
            Self::DEFAULT_TIMEOUT,
        )
        .await
    }
}
