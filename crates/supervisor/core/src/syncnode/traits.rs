use super::ManagedNodeError;
use crate::event::ChainEvent;
use alloy_eips::BlockNumHash;
use alloy_primitives::B256;
use async_trait::async_trait;
use kona_protocol::BlockInfo;
use kona_supervisor_types::{BlockSeal, OutputV0, Receipts};
use std::fmt::Debug;
use tokio::sync::mpsc;

/// Represents a node that can subscribe to L2 events from the chain.
///
/// This trait is responsible for setting up event subscriptions and
/// streaming them through a Tokio MPSC channel. Must be thread-safe.
#[async_trait]
pub trait NodeSubscriber: Send + Sync {
    /// Starts a subscription to the node's event stream.
    ///
    /// # Arguments
    /// * `event_tx` - A Tokio MPSC sender through which [`ChainEvent`]s will be emitted.
    ///
    /// # Returns
    /// * `Ok(())` on successful subscription
    /// * `Err(ManagedNodeError)` if subscription setup fails
    async fn start_subscription(
        &self,
        event_tx: mpsc::Sender<ChainEvent>,
    ) -> Result<(), ManagedNodeError>;
}

/// [`BlockProvider`] abstracts fetching blocks and receipts for a given block.
#[async_trait]
pub trait BlockProvider: Send + Sync + Debug {
    /// Fetch all transaction receipts for the block with the given hash.
    ///
    /// # Arguments
    /// * `block_hash` - The hash of the block whose receipts should be fetched.
    ///
    /// # Returns
    /// [Receipts] representing all transaction receipts in the block,
    /// or an error if the fetch fails.
    async fn fetch_receipts(&self, block_hash: B256) -> Result<Receipts, ManagedNodeError>;

    /// Returns the block info for the given block number
    async fn block_by_number(&self, number: u64) -> Result<BlockInfo, ManagedNodeError>;
}

/// [`ManagedNodeDataProvider`] abstracts the managed node data APIs that supervisor uses to fetch
/// info from the managed node.
#[async_trait]
pub trait ManagedNodeDataProvider: Send + Sync + Debug {
    /// Fetch the output v0 at a given timestamp.
    ///
    /// # Arguments
    /// * `timestamp` - The timestamp to fetch the output v0 at.
    ///
    /// # Returns
    /// The output v0 at the given timestamp,
    /// or an error if the fetch fails.
    async fn output_v0_at_timestamp(&self, timestamp: u64) -> Result<OutputV0, ManagedNodeError>;

    /// Fetch the pending output v0 at a given timestamp.
    ///
    /// # Arguments
    /// * `timestamp` - The timestamp to fetch the pending output v0 at.
    ///
    /// # Returns
    /// The pending output v0 at the given timestamp,
    /// or an error if the fetch fails.
    async fn pending_output_v0_at_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<OutputV0, ManagedNodeError>;

    /// Fetch the l2 block ref by timestamp.
    ///
    /// # Arguments
    /// * `timestamp` - The timestamp to fetch the l2 block ref at.
    ///
    /// # Returns
    /// The l2 block ref at the given timestamp,
    async fn l2_block_ref_by_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<BlockInfo, ManagedNodeError>;
}

/// [`ManagedNodeController`] abstracts the managed node control APIs that supervisor uses to
/// control the managed node state.
#[async_trait]
pub trait ManagedNodeController: Send + Sync + Debug {
    /// Update the finalized block head using the given [`BlockNumHash`].
    ///
    /// # Arguments
    /// * `finalized_block_id` - The block number and hash of the finalized block
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(ManagedNodeError)` if the update fails
    async fn update_finalized(
        &self,
        finalized_block_id: BlockNumHash,
    ) -> Result<(), ManagedNodeError>;

    /// Update the cross unsafe block head using the given [`BlockNumHash`].
    ///
    /// # Arguments
    /// * `cross_unsafe_block_id` - The block number and hash of the cross unsafe block
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(ManagedNodeError)` if the update fails
    async fn update_cross_unsafe(
        &self,
        cross_unsafe_block_id: BlockNumHash,
    ) -> Result<(), ManagedNodeError>;

    /// Update the cross safe block head using the given [`BlockNumHash`].
    ///
    /// # Arguments
    /// * `source_block_id` - The block number and hash of the L1 block
    /// * `derived_block_id` - The block number and hash of the new cross safe block
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(ManagedNodeError)` if the update fails
    async fn update_cross_safe(
        &self,
        source_block_id: BlockNumHash,
        derived_block_id: BlockNumHash,
    ) -> Result<(), ManagedNodeError>;

    /// Reset the managed node based on the supervisor's state.
    /// This is typically used to reset the node's state
    /// when the supervisor detects a misalignment
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(ManagedNodeError)` if the reset fails
    async fn reset(&self) -> Result<(), ManagedNodeError>;

    /// Instructs the managed node to invalidate a block.
    /// This is used when the supervisor detects an invalid block
    /// and needs to roll back the node's state.
    ///
    /// # Arguments
    /// * `seal` - The [`BlockSeal`] of the block.
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(ManagedNodeError)` if the invalidation fails
    async fn invalidate_block(&self, seal: BlockSeal) -> Result<(), ManagedNodeError>;
}

/// Composite trait for any node that provides:
/// - Event subscriptions (`NodeSubscriber`)
/// - Receipt access (`ReceiptProvider`)
/// - Managed node API access (`ManagedNodeApiProvider`)
///
/// This is the main abstraction used for a fully-managed node
/// within the supervisor context.
#[async_trait]
pub trait ManagedNodeProvider:
    NodeSubscriber
    + BlockProvider
    + ManagedNodeDataProvider
    + ManagedNodeController
    + Send
    + Sync
    + Debug
{
}

#[async_trait]
impl<T> ManagedNodeProvider for T where
    T: NodeSubscriber
        + BlockProvider
        + ManagedNodeDataProvider
        + ManagedNodeController
        + Send
        + Sync
        + Debug
{
}
