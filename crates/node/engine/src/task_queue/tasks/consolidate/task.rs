//! A task to consolidate the engine state.

use crate::{
    BuildTask, ConsolidateTaskError, EngineClient, EngineState, EngineTaskError, EngineTaskExt,
    ForkchoiceTask, Metrics,
};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::{L2BlockInfo, OpAttributesWithParent};
use std::{sync::Arc, time::Instant};

/// The [`ConsolidateTask`] attempts to consolidate the engine state
/// using the specified payload attributes and the oldest unsafe head.
///
/// If consolidation fails, payload attributes processing is attempted using the [`BuildTask`].
#[derive(Debug, Clone)]
pub struct ConsolidateTask {
    /// The engine client.
    pub client: Arc<EngineClient>,
    /// The [`RollupConfig`].
    pub cfg: Arc<RollupConfig>,
    /// The [`OpAttributesWithParent`] to instruct the execution layer to build.
    pub attributes: OpAttributesWithParent,
    /// Whether or not the payload was derived, or created by the sequencer.
    pub is_attributes_derived: bool,
}

impl ConsolidateTask {
    /// Creates a new [`ConsolidateTask`].
    pub const fn new(
        client: Arc<EngineClient>,
        config: Arc<RollupConfig>,
        attributes: OpAttributesWithParent,
        is_attributes_derived: bool,
    ) -> Self {
        Self { client, cfg: config, attributes, is_attributes_derived }
    }

    /// Executes the [`ForkchoiceTask`] if the attributes match the block.
    async fn execute_forkchoice_task(
        &self,
        state: &mut EngineState,
    ) -> Result<(), EngineTaskError> {
        let task = ForkchoiceTask::new(Arc::clone(&self.client));
        task.execute(state).await
    }

    /// Executes a new [`BuildTask`].
    /// This is used when the [`ConsolidateTask`] fails to consolidate the engine state.
    async fn execute_build_task(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        let build_task = BuildTask::new(
            self.client.clone(),
            self.cfg.clone(),
            self.attributes.clone(),
            self.is_attributes_derived,
            None,
        );
        build_task.execute(state).await
    }

    /// Attempts consolidation on the engine state.
    pub async fn consolidate(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        let global_start = Instant::now();

        // Fetch the unsafe l2 block after the attributes parent.
        let block_num = self.attributes.block_number();
        let fetch_start = Instant::now();
        let block = match self.client.l2_block_by_label(block_num.into()).await {
            Ok(Some(block)) => block,
            Ok(None) => {
                warn!(target: "engine", "Received `None` block for {}", block_num);
                return Err(ConsolidateTaskError::MissingUnsafeL2Block(block_num).into());
            }
            Err(_) => {
                warn!(target: "engine", "Failed to fetch unsafe l2 block for consolidation");
                return Err(ConsolidateTaskError::FailedToFetchUnsafeL2Block.into());
            }
        };
        let block_fetch_duration = fetch_start.elapsed();

        // Attempt to consolidate the unsafe head.
        // If this is successful, the forkchoice change synchronizes.
        // Otherwise, the attributes need to be processed.
        let block_hash = block.header.hash;
        if crate::AttributesMatch::check(&self.cfg, &self.attributes, &block).is_match() {
            trace!(
                target: "engine",
                attributes = ?self.attributes,
                block_hash = %block_hash,
                "Consolidating engine state",
            );

            match L2BlockInfo::from_block_and_genesis(&block.into_consensus(), &self.cfg.genesis) {
                Ok(block_info) => {
                    state.set_local_safe_head(block_info);
                    state.set_safe_head(block_info);

                    // Only issue a forkchoice update if the attributes are the last in the span
                    // batch. This is an optimization to avoid sending a FCU
                    // call for every block in the span batch.
                    let fcu_duration = if self.attributes.is_last_in_span {
                        let fcu_start = Instant::now();
                        if let Err(e) = self.execute_forkchoice_task(state).await {
                            warn!(target: "engine", ?e, "Consolidation failed");
                            return Err(e);
                        }
                        Some(fcu_start.elapsed())
                    } else {
                        None
                    };

                    let total_duration = global_start.elapsed();

                    // Update metrics.
                    kona_macros::inc!(
                        counter,
                        Metrics::ENGINE_TASK_COUNT,
                        Metrics::CONSOLIDATE_TASK_LABEL
                    );

                    info!(
                        target: "engine",
                        hash = %block_info.block_info.hash,
                        number = block_info.block_info.number,
                        ?total_duration,
                        ?block_fetch_duration,
                        fcu_duration = %fcu_duration.map(|d| format!("{d:?}")).unwrap_or("N/A".to_string()),
                        "Updated safe head via L1 consolidation"
                    );

                    return Ok(());
                }
                Err(e) => {
                    // Continue on to build the block since we failed to construct the block info.
                    warn!(target: "engine", ?e, "Failed to construct L2BlockInfo, proceeding to build task");
                }
            }
        }

        // Otherwise, the attributes need to be processed.
        debug!(
            target: "engine",
            attributes = ?self.attributes,
            block_hash = %block_hash,
            "Attributes mismatch! Executing build task to initiate reorg",
        );
        self.execute_build_task(state).await
    }
}

#[async_trait]
impl EngineTaskExt for ConsolidateTask {
    async fn execute(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        // Skip to building the payload attributes if consolidation is not needed.
        if state.safe_head().block_info.number < state.unsafe_head().block_info.number {
            self.consolidate(state).await
        } else {
            self.execute_build_task(state).await
        }
    }
}
