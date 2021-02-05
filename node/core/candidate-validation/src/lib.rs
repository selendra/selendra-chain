// Copyright 2020 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! The Candidate Validation subsystem.
//!
//! This handles incoming requests from other subsystems to validate candidates
//! according to a validation function. This delegates validation to an underlying
//! pool of processes used for execution of the Wasm.

#![deny(unused_crate_dependencies, unused_results)]
#![warn(missing_docs)]

use indracore_subsystem::{
	Subsystem, SubsystemContext, SpawnedSubsystem, SubsystemResult, SubsystemError,
	FromOverseer, OverseerSignal,
	messages::{
		AllMessages, CandidateValidationMessage, RuntimeApiMessage,
		ValidationFailed, RuntimeApiRequest,
	},
};
use indracore_node_subsystem_util::metrics::{self, prometheus};
use indracore_subsystem::errors::RuntimeApiError;
use indracore_node_primitives::{ValidationResult, InvalidCandidate};
use indracore_primitives::v1::{
	ValidationCode, PoV, CandidateDescriptor, PersistedValidationData,
	OccupiedCoreAssumption, Hash, CandidateCommitments,
};
use indracore_parachain::wasm_executor::{
	self, IsolationStrategy, ValidationError, InvalidCandidate as WasmInvalidCandidate
};
use indracore_parachain::primitives::{ValidationResult as WasmValidationResult, ValidationParams};

use parity_scale_codec::Encode;
use sp_core::traits::SpawnNamed;

use futures::channel::oneshot;
use futures::prelude::*;

use std::sync::Arc;

const LOG_TARGET: &'static str = "candidate_validation";

/// The candidate validation subsystem.
pub struct CandidateValidationSubsystem<S> {
	spawn: S,
	metrics: Metrics,
	isolation_strategy: IsolationStrategy,
}

impl<S> CandidateValidationSubsystem<S> {
	/// Create a new `CandidateValidationSubsystem` with the given task spawner and isolation
	/// strategy.
	///
	/// Check out [`IsolationStrategy`] to get more details.
	pub fn new(spawn: S, metrics: Metrics, isolation_strategy: IsolationStrategy) -> Self {
		CandidateValidationSubsystem { spawn, metrics, isolation_strategy }
	}
}

impl<S, C> Subsystem<C> for CandidateValidationSubsystem<S> where
	C: SubsystemContext<Message = CandidateValidationMessage>,
	S: SpawnNamed + Clone + 'static,
{
	fn start(self, ctx: C) -> SpawnedSubsystem {
		let future = run(ctx, self.spawn, self.metrics, self.isolation_strategy)
			.map_err(|e| SubsystemError::with_origin("candidate-validation", e))
			.boxed();
		SpawnedSubsystem {
			name: "candidate-validation-subsystem",
			future,
		}
	}
}

#[tracing::instrument(skip(ctx, spawn, metrics), fields(subsystem = LOG_TARGET))]
async fn run(
	mut ctx: impl SubsystemContext<Message = CandidateValidationMessage>,
	spawn: impl SpawnNamed + Clone + 'static,
	metrics: Metrics,
	isolation_strategy: IsolationStrategy,
) -> SubsystemResult<()> {
	loop {
		match ctx.recv().await? {
			FromOverseer::Signal(OverseerSignal::ActiveLeaves(_)) => {}
			FromOverseer::Signal(OverseerSignal::BlockFinalized(_)) => {}
			FromOverseer::Signal(OverseerSignal::Conclude) => return Ok(()),
			FromOverseer::Communication { msg } => match msg {
				CandidateValidationMessage::ValidateFromChainState(
					descriptor,
					pov,
					response_sender,
				) => {
					let _timer = metrics.time_validate_from_chain_state();

					let res = spawn_validate_from_chain_state(
						&mut ctx,
						isolation_strategy.clone(),
						descriptor,
						pov,
						spawn.clone(),
						&metrics,
					).await;

					match res {
						Ok(x) => {
							metrics.on_validation_event(&x);
							let _ = response_sender.send(x);
						}
						Err(e) => return Err(e),
					}
				}
				CandidateValidationMessage::ValidateFromExhaustive(
					persisted_validation_data,
					validation_code,
					descriptor,
					pov,
					response_sender,
				) => {
					let _timer = metrics.time_validate_from_exhaustive();

					let res = spawn_validate_exhaustive(
						&mut ctx,
						isolation_strategy.clone(),
						persisted_validation_data,
						validation_code,
						descriptor,
						pov,
						spawn.clone(),
						&metrics,
					).await;

					match res {
						Ok(x) => {
							metrics.on_validation_event(&x);
							if let Err(_e) = response_sender.send(x) {
								tracing::warn!(
									target: LOG_TARGET,
									"Requester of candidate validation dropped",
								)
							}
						},
						Err(e) => return Err(e),
					}
				}
			}
		}
	}
}

async fn runtime_api_request<T>(
	ctx: &mut impl SubsystemContext<Message = CandidateValidationMessage>,
	relay_parent: Hash,
	request: RuntimeApiRequest,
	receiver: oneshot::Receiver<Result<T, RuntimeApiError>>,
) -> SubsystemResult<Result<T, RuntimeApiError>> {
	ctx.send_message(
		AllMessages::RuntimeApi(RuntimeApiMessage::Request(
			relay_parent,
			request,
		))
	).await;

	receiver.await.map_err(Into::into)
}

#[derive(Debug)]
enum AssumptionCheckOutcome {
	Matches(PersistedValidationData, ValidationCode),
	DoesNotMatch,
	BadRequest,
}

#[tracing::instrument(level = "trace", skip(ctx), fields(subsystem = LOG_TARGET))]
async fn check_assumption_validation_data(
	ctx: &mut impl SubsystemContext<Message = CandidateValidationMessage>,
	descriptor: &CandidateDescriptor,
	assumption: OccupiedCoreAssumption,
) -> SubsystemResult<AssumptionCheckOutcome> {
	let validation_data = {
		let (tx, rx) = oneshot::channel();
		let d = runtime_api_request(
			ctx,
			descriptor.relay_parent,
			RuntimeApiRequest::PersistedValidationData(
				descriptor.para_id,
				assumption,
				tx,
			),
			rx,
		).await?;

		match d {
			Ok(None) | Err(_) => {
				return Ok(AssumptionCheckOutcome::BadRequest);
			}
			Ok(Some(d)) => d,
		}
	};

	let persisted_validation_data_hash = validation_data.hash();

	SubsystemResult::Ok(if descriptor.persisted_validation_data_hash == persisted_validation_data_hash {
		let (code_tx, code_rx) = oneshot::channel();
		let validation_code = runtime_api_request(
			ctx,
			descriptor.relay_parent,
			RuntimeApiRequest::ValidationCode(
				descriptor.para_id,
				assumption,
				code_tx,
			),
			code_rx,
		).await?;

		match validation_code {
			Ok(None) | Err(_) => AssumptionCheckOutcome::BadRequest,
			Ok(Some(v)) => AssumptionCheckOutcome::Matches(validation_data, v),
		}
	} else {
		AssumptionCheckOutcome::DoesNotMatch
	})
}

#[tracing::instrument(level = "trace", skip(ctx), fields(subsystem = LOG_TARGET))]
async fn find_assumed_validation_data(
	ctx: &mut impl SubsystemContext<Message = CandidateValidationMessage>,
	descriptor: &CandidateDescriptor,
) -> SubsystemResult<AssumptionCheckOutcome> {
	// The candidate descriptor has a `persisted_validation_data_hash` which corresponds to
	// one of up to two possible values that we can derive from the state of the
	// relay-parent. We can fetch these values by getting the persisted validation data
	// based on the different `OccupiedCoreAssumption`s.

	const ASSUMPTIONS: &[OccupiedCoreAssumption] = &[
		OccupiedCoreAssumption::Included,
		OccupiedCoreAssumption::TimedOut,
		// `TimedOut` and `Free` both don't perform any speculation and therefore should be the same
		// for our purposes here. In other words, if `TimedOut` matched then the `Free` must be
		// matched as well.
	];

	// Consider running these checks in parallel to reduce validation latency.
	for assumption in ASSUMPTIONS {
		let outcome = check_assumption_validation_data(ctx, descriptor, *assumption).await?;

		match outcome {
			AssumptionCheckOutcome::Matches(_, _) => return Ok(outcome),
			AssumptionCheckOutcome::BadRequest => return Ok(outcome),
			AssumptionCheckOutcome::DoesNotMatch => continue,
		}
	}

	Ok(AssumptionCheckOutcome::DoesNotMatch)
}

#[tracing::instrument(level = "trace", skip(ctx, pov, spawn, metrics), fields(subsystem = LOG_TARGET))]
async fn spawn_validate_from_chain_state(
	ctx: &mut impl SubsystemContext<Message = CandidateValidationMessage>,
	isolation_strategy: IsolationStrategy,
	descriptor: CandidateDescriptor,
	pov: Arc<PoV>,
	spawn: impl SpawnNamed + 'static,
	metrics: &Metrics,
) -> SubsystemResult<Result<ValidationResult, ValidationFailed>> {
	let (validation_data, validation_code) =
		match find_assumed_validation_data(ctx, &descriptor).await? {
			AssumptionCheckOutcome::Matches(validation_data, validation_code) => {
				(validation_data, validation_code)
			}
			AssumptionCheckOutcome::DoesNotMatch => {
				// If neither the assumption of the occupied core having the para included or the assumption
				// of the occupied core timing out are valid, then the persisted_validation_data_hash in the descriptor
				// is not based on the relay parent and is thus invalid.
				return Ok(Ok(ValidationResult::Invalid(InvalidCandidate::BadParent)));
			}
			AssumptionCheckOutcome::BadRequest => {
				return Ok(Err(ValidationFailed("Assumption Check: Bad request".into())));
			}
		};

	let validation_result = spawn_validate_exhaustive(
		ctx,
		isolation_strategy,
		validation_data,
		validation_code,
		descriptor.clone(),
		pov,
		spawn,
		metrics,
	)
	.await;

	if let Ok(Ok(ValidationResult::Valid(ref outputs, _))) = validation_result {
		let (tx, rx) = oneshot::channel();
		match runtime_api_request(
			ctx,
			descriptor.relay_parent,
			RuntimeApiRequest::CheckValidationOutputs(descriptor.para_id, outputs.clone(), tx),
			rx,
		)
		.await?
		{
			Ok(true) => {}
			Ok(false) => {
				return Ok(Ok(ValidationResult::Invalid(
					InvalidCandidate::InvalidOutputs,
				)));
			}
			Err(_) => {
				return Ok(Err(ValidationFailed("Check Validation Outputs: Bad request".into())));
			}
		}
	}

	validation_result
}

#[tracing::instrument(level = "trace", skip(ctx, validation_code, pov, spawn, metrics), fields(subsystem = LOG_TARGET))]
async fn spawn_validate_exhaustive(
	ctx: &mut impl SubsystemContext<Message = CandidateValidationMessage>,
	isolation_strategy: IsolationStrategy,
	persisted_validation_data: PersistedValidationData,
	validation_code: ValidationCode,
	descriptor: CandidateDescriptor,
	pov: Arc<PoV>,
	spawn: impl SpawnNamed + 'static,
	metrics: &Metrics,
) -> SubsystemResult<Result<ValidationResult, ValidationFailed>> {
	let (tx, rx) = oneshot::channel();
	let metrics = metrics.clone();
	let fut = async move {
		let res = validate_candidate_exhaustive::<RealValidationBackend, _>(
			isolation_strategy,
			persisted_validation_data,
			validation_code,
			descriptor,
			pov,
			spawn,
			&metrics,
		);

		let _ = tx.send(res);
	};

	ctx.spawn_blocking("blocking-candidate-validation-task", fut.boxed()).await?;
	rx.await.map_err(Into::into)
}

/// Does basic checks of a candidate. Provide the encoded PoV-block. Returns `Ok` if basic checks
/// are passed, `Err` otherwise.
#[tracing::instrument(level = "trace", skip(pov), fields(subsystem = LOG_TARGET))]
fn perform_basic_checks(
	candidate: &CandidateDescriptor,
	max_pov_size: u32,
	pov: &PoV,
) -> Result<(), InvalidCandidate> {
	let encoded_pov = pov.encode();
	let hash = pov.hash();

	if encoded_pov.len() > max_pov_size as usize {
		return Err(InvalidCandidate::ParamsTooLarge(encoded_pov.len() as u64));
	}

	if hash != candidate.pov_hash {
		return Err(InvalidCandidate::HashMismatch);
	}

	if let Err(()) = candidate.check_collator_signature() {
		return Err(InvalidCandidate::BadSignature);
	}

	Ok(())
}

trait ValidationBackend {
	type Arg;

	fn validate<S: SpawnNamed + 'static>(
		arg: Self::Arg,
		validation_code: &ValidationCode,
		params: ValidationParams,
		spawn: S,
	) -> Result<WasmValidationResult, ValidationError>;
}

struct RealValidationBackend;

impl ValidationBackend for RealValidationBackend {
	type Arg = IsolationStrategy;

	fn validate<S: SpawnNamed + 'static>(
		isolation_strategy: IsolationStrategy,
		validation_code: &ValidationCode,
		params: ValidationParams,
		spawn: S,
	) -> Result<WasmValidationResult, ValidationError> {
		wasm_executor::validate_candidate(
			&validation_code.0,
			params,
			&isolation_strategy,
			spawn,
		)
	}
}

/// Validates the candidate from exhaustive parameters.
///
/// Sends the result of validation on the channel once complete.
#[tracing::instrument(level = "trace", skip(backend_arg, validation_code, pov, spawn, metrics), fields(subsystem = LOG_TARGET))]
fn validate_candidate_exhaustive<B: ValidationBackend, S: SpawnNamed + 'static>(
	backend_arg: B::Arg,
	persisted_validation_data: PersistedValidationData,
	validation_code: ValidationCode,
	descriptor: CandidateDescriptor,
	pov: Arc<PoV>,
	spawn: S,
	metrics: &Metrics,
) -> Result<ValidationResult, ValidationFailed> {
	let _timer = metrics.time_validate_candidate_exhaustive();

	if let Err(e) = perform_basic_checks(&descriptor, persisted_validation_data.max_pov_size, &*pov) {
		return Ok(ValidationResult::Invalid(e))
	}

	let params = ValidationParams {
		parent_head: persisted_validation_data.parent_head.clone(),
		block_data: pov.block_data.clone(),
		relay_chain_height: persisted_validation_data.block_number,
		dmq_mqc_head: persisted_validation_data.dmq_mqc_head,
		hrmp_mqc_heads: persisted_validation_data.hrmp_mqc_heads.clone(),
	};

	match B::validate(backend_arg, &validation_code, params, spawn) {
		Err(ValidationError::InvalidCandidate(WasmInvalidCandidate::Timeout)) =>
			Ok(ValidationResult::Invalid(InvalidCandidate::Timeout)),
		Err(ValidationError::InvalidCandidate(WasmInvalidCandidate::ParamsTooLarge(l))) =>
			Ok(ValidationResult::Invalid(InvalidCandidate::ParamsTooLarge(l as u64))),
		Err(ValidationError::InvalidCandidate(WasmInvalidCandidate::CodeTooLarge(l))) =>
			Ok(ValidationResult::Invalid(InvalidCandidate::CodeTooLarge(l as u64))),
		Err(ValidationError::InvalidCandidate(WasmInvalidCandidate::BadReturn)) =>
			Ok(ValidationResult::Invalid(InvalidCandidate::BadReturn)),
		Err(ValidationError::InvalidCandidate(WasmInvalidCandidate::WasmExecutor(e))) =>
			Ok(ValidationResult::Invalid(InvalidCandidate::ExecutionError(e.to_string()))),
		Err(ValidationError::InvalidCandidate(WasmInvalidCandidate::ExternalWasmExecutor(e))) =>
			Ok(ValidationResult::Invalid(InvalidCandidate::ExecutionError(e.to_string()))),
		Err(ValidationError::Internal(e)) => Err(ValidationFailed(e.to_string())),
		Ok(res) => {
			let outputs = CandidateCommitments {
				head_data: res.head_data,
				upward_messages: res.upward_messages,
				horizontal_messages: res.horizontal_messages,
				new_validation_code: res.new_validation_code,
				processed_downward_messages: res.processed_downward_messages,
				hrmp_watermark: res.hrmp_watermark,
			};
			Ok(ValidationResult::Valid(outputs, persisted_validation_data))
		}
	}
}

#[derive(Clone)]
struct MetricsInner {
	validation_requests: prometheus::CounterVec<prometheus::U64>,
	validate_from_chain_state: prometheus::Histogram,
	validate_from_exhaustive: prometheus::Histogram,
	validate_candidate_exhaustive: prometheus::Histogram,
}

/// Candidate validation metrics.
#[derive(Default, Clone)]
pub struct Metrics(Option<MetricsInner>);

impl Metrics {
	fn on_validation_event(&self, event: &Result<ValidationResult, ValidationFailed>) {
		if let Some(metrics) = &self.0 {
			match event {
				Ok(ValidationResult::Valid(_, _)) => {
					metrics.validation_requests.with_label_values(&["valid"]).inc();
				},
				Ok(ValidationResult::Invalid(_)) => {
					metrics.validation_requests.with_label_values(&["invalid"]).inc();
				},
				Err(_) => {
					metrics.validation_requests.with_label_values(&["validation failure"]).inc();
				},
			}
		}
	}

	/// Provide a timer for `validate_from_chain_state` which observes on drop.
	fn time_validate_from_chain_state(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.validate_from_chain_state.start_timer())
	}

	/// Provide a timer for `validate_from_exhaustive` which observes on drop.
	fn time_validate_from_exhaustive(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.validate_from_exhaustive.start_timer())
	}

	/// Provide a timer for `validate_candidate_exhaustive` which observes on drop.
	fn time_validate_candidate_exhaustive(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.validate_candidate_exhaustive.start_timer())
	}
}

impl metrics::Metrics for Metrics {
	fn try_register(registry: &prometheus::Registry) -> Result<Self, prometheus::PrometheusError> {
		let metrics = MetricsInner {
			validation_requests: prometheus::register(
				prometheus::CounterVec::new(
					prometheus::Opts::new(
						"parachain_validation_requests_total",
						"Number of validation requests served.",
					),
					&["validity"],
				)?,
				registry,
			)?,
			validate_from_chain_state: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_candidate_validation_validate_from_chain_state",
						"Time spent within `candidate_validation::validate_from_chain_state`",
					)
				)?,
				registry,
			)?,
			validate_from_exhaustive: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_candidate_validation_validate_from_exhaustive",
						"Time spent within `candidate_validation::validate_from_exhaustive`",
					)
				)?,
				registry,
			)?,
			validate_candidate_exhaustive: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_candidate_validation_validate_candidate_exhaustive",
						"Time spent within `candidate_validation::validate_candidate_exhaustive`",
					)
				)?,
				registry,
			)?,
		};
		Ok(Metrics(Some(metrics)))
	}
}