// Copyright 2019-2021 Parity Technologies (UK) Ltd.
// This file is part of Cumulus.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

//! Cumulus Collator implementation for Substrate.

use cumulus_client_network::WaitToAnnounce;
use cumulus_primitives_core::{
    well_known_keys, OutboundHrmpMessage, ParachainBlockData, PersistedValidationData,
};

use sc_client_api::BlockBackend;
use sp_consensus::BlockStatus;
use sp_core::traits::SpawnNamed;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, Header as HeaderT, Zero},
};
use sp_state_machine::InspectState;

use cumulus_client_consensus_common::ParachainConsensus;
use indracore_node_primitives::{Collation, CollationGenerationConfig, CollationResult};
use indracore_node_subsystem::messages::{CollationGenerationMessage, CollatorProtocolMessage};
use indracore_overseer::OverseerHandler;
use indracore_primitives::v1::{
    BlockData, BlockNumber as PBlockNumber, CollatorPair, Hash as PHash, HeadData, Id as ParaId,
    PoV, UpwardMessage,
};

use codec::{Decode, Encode};
use futures::{channel::oneshot, FutureExt};
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::Instrument;

/// The logging target.
const LOG_TARGET: &str = "cumulus-collator";

/// The implementation of the Cumulus `Collator`.
pub struct Collator<Block: BlockT, BS, Backend> {
    block_status: Arc<BS>,
    parachain_consensus: Box<dyn ParachainConsensus<Block>>,
    wait_to_announce: Arc<Mutex<WaitToAnnounce<Block>>>,
    backend: Arc<Backend>,
}

impl<Block: BlockT, BS, Backend> Clone for Collator<Block, BS, Backend> {
    fn clone(&self) -> Self {
        Self {
            block_status: self.block_status.clone(),
            wait_to_announce: self.wait_to_announce.clone(),
            backend: self.backend.clone(),
            parachain_consensus: self.parachain_consensus.clone(),
        }
    }
}

impl<Block, BS, Backend> Collator<Block, BS, Backend>
where
    Block: BlockT,
    BS: BlockBackend<Block>,
    Backend: sc_client_api::Backend<Block> + 'static,
{
    /// Create a new instance.
    fn new(
        block_status: Arc<BS>,
        spawner: Arc<dyn SpawnNamed + Send + Sync>,
        announce_block: Arc<dyn Fn(Block::Hash, Option<Vec<u8>>) + Send + Sync>,
        backend: Arc<Backend>,
        parachain_consensus: Box<dyn ParachainConsensus<Block>>,
    ) -> Self {
        let wait_to_announce = Arc::new(Mutex::new(WaitToAnnounce::new(spawner, announce_block)));

        Self {
            block_status,
            wait_to_announce,
            backend,
            parachain_consensus,
        }
    }

    /// Checks the status of the given block hash in the Parachain.
    ///
    /// Returns `true` if the block could be found and is good to be build on.
    fn check_block_status(&self, hash: Block::Hash, header: &Block::Header) -> bool {
        match self.block_status.block_status(&BlockId::Hash(hash)) {
            Ok(BlockStatus::Queued) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    block_hash = ?hash,
                    "Skipping candidate production, because block is still queued for import.",
                );
                false
            }
            Ok(BlockStatus::InChainWithState) => true,
            Ok(BlockStatus::InChainPruned) => {
                tracing::error!(
                    target: LOG_TARGET,
                    "Skipping candidate production, because block `{:?}` is already pruned!",
                    hash,
                );
                false
            }
            Ok(BlockStatus::KnownBad) => {
                tracing::error!(
                    target: LOG_TARGET,
                    block_hash = ?hash,
                    "Block is tagged as known bad and is included in the relay chain! Skipping candidate production!",
                );
                false
            }
            Ok(BlockStatus::Unknown) => {
                if header.number().is_zero() {
                    tracing::error!(
                        target: LOG_TARGET,
                        block_hash = ?hash,
                        "Could not find the header of the genesis block in the database!",
                    );
                } else {
                    tracing::debug!(
                        target: LOG_TARGET,
                        block_hash = ?hash,
                        "Skipping candidate production, because block is unknown.",
                    );
                }
                false
            }
            Err(e) => {
                tracing::error!(
                    target: LOG_TARGET,
                    block_hash = ?hash,
                    error = ?e,
                    "Failed to get block status.",
                );
                false
            }
        }
    }

    fn build_collation(
        &mut self,
        block: ParachainBlockData<Block>,
        block_hash: Block::Hash,
        relay_block_number: PBlockNumber,
    ) -> Option<Collation> {
        let block_data = BlockData(block.encode());
        let header = block.into_header();
        let head_data = HeadData(header.encode());

        let state = match self.backend.state_at(BlockId::Hash(block_hash)) {
            Ok(state) => state,
            Err(e) => {
                tracing::error!(
                    target: LOG_TARGET,
                    error = ?e,
                    "Failed to get state of the freshly built block.",
                );
                return None;
            }
        };

        state.inspect_state(|| {
            let upward_messages = sp_io::storage::get(well_known_keys::UPWARD_MESSAGES);
            let upward_messages =
                match upward_messages.map(|v| Vec::<UpwardMessage>::decode(&mut &v[..])) {
                    Some(Ok(msgs)) => msgs,
                    Some(Err(e)) => {
                        tracing::error!(
                            target: LOG_TARGET,
                            error = ?e,
                            "Failed to decode upward messages from the build block.",
                        );
                        return None;
                    }
                    None => Vec::new(),
                };

            let new_validation_code = sp_io::storage::get(well_known_keys::NEW_VALIDATION_CODE);

            let processed_downward_messages =
                sp_io::storage::get(well_known_keys::PROCESSED_DOWNWARD_MESSAGES);
            let processed_downward_messages =
                match processed_downward_messages.map(|v| u32::decode(&mut &v[..])) {
                    Some(Ok(processed_cnt)) => processed_cnt,
                    Some(Err(e)) => {
                        tracing::error!(
                            target: LOG_TARGET,
                            error = ?e,
                            "Failed to decode the count of processed downward message.",
                        );
                        return None;
                    }
                    None => 0,
                };

            let horizontal_messages = sp_io::storage::get(well_known_keys::HRMP_OUTBOUND_MESSAGES);
            let horizontal_messages = match horizontal_messages
                .map(|v| Vec::<OutboundHrmpMessage>::decode(&mut &v[..]))
            {
                Some(Ok(horizontal_messages)) => horizontal_messages,
                Some(Err(e)) => {
                    tracing::error!(
                        target: LOG_TARGET,
                        error = ?e,
                        "Failed to decode the horizontal messages.",
                    );
                    return None;
                }
                None => Vec::new(),
            };

            let hrmp_watermark = sp_io::storage::get(well_known_keys::HRMP_WATERMARK);
            let hrmp_watermark = match hrmp_watermark.map(|v| PBlockNumber::decode(&mut &v[..])) {
                Some(Ok(hrmp_watermark)) => hrmp_watermark,
                Some(Err(e)) => {
                    tracing::error!(
                        target: LOG_TARGET,
                        error = ?e,
                        "Failed to decode the HRMP watermark."
                    );
                    return None;
                }
                None => {
                    // If the runtime didn't set `HRMP_WATERMARK`, then it means no messages were
                    // supplied via the message ingestion inherent. Assuming that the PVF/runtime
                    // checks that legitly there are no pending messages we can therefore move the
                    // watermark up to the relay-block number.
                    relay_block_number
                }
            };

            Some(Collation {
                upward_messages,
                new_validation_code: new_validation_code.map(Into::into),
                head_data,
                proof_of_validity: PoV { block_data },
                processed_downward_messages,
                horizontal_messages,
                hrmp_watermark,
            })
        })
    }

    async fn produce_candidate(
        mut self,
        relay_parent: PHash,
        validation_data: PersistedValidationData,
    ) -> Option<CollationResult> {
        tracing::trace!(
            target: LOG_TARGET,
            relay_parent = ?relay_parent,
            "Producing candidate",
        );

        let last_head = match Block::Header::decode(&mut &validation_data.parent_head.0[..]) {
            Ok(x) => x,
            Err(e) => {
                tracing::error!(
                    target: LOG_TARGET,
                    error = ?e,
                    "Could not decode the head data."
                );
                return None;
            }
        };

        let last_head_hash = last_head.hash();
        if !self.check_block_status(last_head_hash, &last_head) {
            return None;
        }

        tracing::info!(
            target: LOG_TARGET,
            relay_parent = ?relay_parent,
            at = ?last_head_hash,
            "Starting collation.",
        );

        let candidate = self
            .parachain_consensus
            .produce_candidate(&last_head, relay_parent, &validation_data)
            .await?;

        let (header, extrinsics) = candidate.block.deconstruct();

        // Create the parachain block data for the validators.
        let b = ParachainBlockData::<Block>::new(header, extrinsics, candidate.proof);

        tracing::debug!(
            target: LOG_TARGET,
            "PoV size {{ header: {}kb, extrinsics: {}kb, storage_proof: {}kb }}",
            b.header().encode().len() as f64 / 1024f64,
            b.extrinsics().encode().len() as f64 / 1024f64,
            b.storage_proof().encode().len() as f64 / 1024f64,
        );

        let block_hash = b.header().hash();
        let collation = self.build_collation(b, block_hash, validation_data.relay_parent_number)?;
        let pov_hash = collation.proof_of_validity.hash();

        let (result_sender, signed_stmt_recv) = oneshot::channel();

        self.wait_to_announce
            .lock()
            .wait_to_announce(block_hash, pov_hash, signed_stmt_recv);

        tracing::info!(
            target: LOG_TARGET,
            pov_hash = ?pov_hash,
            ?block_hash,
            "Produced proof-of-validity candidate.",
        );

        Some(CollationResult {
            collation,
            result_sender: Some(result_sender),
        })
    }
}

/// Parameters for [`start_collator`].
pub struct StartCollatorParams<Block: BlockT, Backend, BS, Spawner> {
    pub para_id: ParaId,
    pub backend: Arc<Backend>,
    pub block_status: Arc<BS>,
    pub announce_block: Arc<dyn Fn(Block::Hash, Option<Vec<u8>>) + Send + Sync>,
    pub overseer_handler: OverseerHandler,
    pub spawner: Spawner,
    pub key: CollatorPair,
    pub parachain_consensus: Box<dyn ParachainConsensus<Block>>,
}

/// Start the collator.
pub async fn start_collator<Block, Backend, BS, Spawner>(
    StartCollatorParams {
        para_id,
        block_status,
        announce_block,
        mut overseer_handler,
        spawner,
        key,
        parachain_consensus,
        backend,
    }: StartCollatorParams<Block, Backend, BS, Spawner>,
) where
    Block: BlockT,
    Backend: sc_client_api::Backend<Block> + 'static,
    BS: BlockBackend<Block> + Send + Sync + 'static,
    Spawner: SpawnNamed + Clone + Send + Sync + 'static,
{
    let collator = Collator::new(
        block_status,
        Arc::new(spawner),
        announce_block,
        backend,
        parachain_consensus,
    );

    let span = tracing::Span::current();
    let config = CollationGenerationConfig {
        key,
        para_id,
        collator: Box::new(move |relay_parent, validation_data| {
            let collator = collator.clone();
            collator
                .produce_candidate(relay_parent, validation_data.clone())
                .instrument(span.clone())
                .boxed()
        }),
    };

    overseer_handler
        .send_msg(CollationGenerationMessage::Initialize(config))
        .await;

    overseer_handler
        .send_msg(CollatorProtocolMessage::CollateOn(para_id))
        .await;
}
