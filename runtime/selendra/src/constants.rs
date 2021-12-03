// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

/// Money matters.
pub mod currency {
	use primitives::v0::Balance;

	pub const UNITS: Balance = 1_000_000_000_000_000_000;
	pub const CENTS: Balance = UNITS / 10_000;
	pub const MILLICENTS: Balance = CENTS / 1_000;
	pub const NANO: Balance = MILLICENTS / 1000;

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 5_000 * CENTS + (bytes as Balance) * 50 * MILLICENTS
	}
}

/// Time and blocks.
pub mod time {
	use primitives::v0::{BlockNumber, Moment};
	pub const MILLISECS_PER_BLOCK: Moment = 6000;
	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;
	pub const EPOCH_DURATION_IN_SLOTS: BlockNumber = 4 * HOURS;

	// These time units are defined in number of blocks.
	pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;
	pub const WEEKS: BlockNumber = DAYS * 7;

	// 1 in 4 blocks (on average, not counting collisions) will be primary babe blocks.
	pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);
}

/// Fee-related.
pub mod fee {
	use frame_support::weights::{
		WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
	};
	use primitives::v0::Balance;
	use runtime_common::ExtrinsicBaseWeight;
	use smallvec::smallvec;
	pub use sp_runtime::Perbill;

	/// The block saturation level. Fees will be updates based on this value.
	pub const TARGET_BLOCK_FULLNESS: Perbill = Perbill::from_percent(25);

	/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
	/// node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - [0, `MAXIMUM_BLOCK_WEIGHT`]
	///   - [Balance::min, Balance::max]
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			let p = 100 * super::currency::MILLICENTS;
			let q = 10 * Balance::from(ExtrinsicBaseWeight::get());
			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_rational(p % q, q),
				coeff_integer: p / q,
			}]
		}
	}
}

pub mod merge_account {
	use crate::Balances;
	use frame_support::{traits::ReservableCurrency, transactional};
	use pallet_evm_accounts::account::MergeAccount;
	use primitives::v1::AccountId;
	use sp_runtime::DispatchResult;

	pub struct MergeAccountEvm;
	impl MergeAccount<AccountId> for MergeAccountEvm {
		#[transactional]
		fn merge_account(source: &AccountId, dest: &AccountId) -> DispatchResult {
			// unreserve all reserved currency
			<Balances as ReservableCurrency<_>>::unreserve(
				source,
				Balances::reserved_balance(source),
			);

			// transfer all free to dest
			match Balances::transfer(
				Some(source.clone()).into(),
				dest.clone().into(),
				Balances::free_balance(source),
			) {
				Ok(_) => Ok(()),
				Err(e) => Err(e.error),
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::{
		currency::{CENTS, MILLICENTS},
		fee::WeightToFee,
	};
	use frame_support::weights::WeightToFeePolynomial;
	use runtime_common::{ExtrinsicBaseWeight, MAXIMUM_BLOCK_WEIGHT};

	#[test]
	// This function tests that the fee for `MAXIMUM_BLOCK_WEIGHT` of weight is correct
	fn full_block_fee_is_correct() {
		// A full block should cost 1,600 CENTS
		println!("Base: {}", ExtrinsicBaseWeight::get());
		let x = WeightToFee::calc(&MAXIMUM_BLOCK_WEIGHT);
		let y = 16 * 100 * CENTS;
		assert!(x.max(y) - x.min(y) < MILLICENTS);
	}

	#[test]
	// This function tests that the fee for `ExtrinsicBaseWeight` of weight is correct
	fn extrinsic_base_fee_is_correct() {
		// `ExtrinsicBaseWeight` should cost 1/10 of a CENT
		println!("Base: {}", ExtrinsicBaseWeight::get());
		let x = WeightToFee::calc(&ExtrinsicBaseWeight::get());
		let y = CENTS / 10;
		assert!(x.max(y) - x.min(y) < MILLICENTS);
	}
}

pub mod precompiles {
	use pallet_evm::{Context, Precompile, PrecompileResult, PrecompileSet};
	use sp_core::H160;
	use sp_std::marker::PhantomData;

	use pallet_evm_precompile_blake2::Blake2F;
	use pallet_evm_precompile_bn128::{Bn128Add, Bn128Mul, Bn128Pairing};
	use pallet_evm_precompile_modexp::Modexp;
	use pallet_evm_precompile_sha3fips::Sha3FIPS256;
	use pallet_evm_precompile_simple::{
		ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256,
	};

	pub struct FrontierPrecompiles<R>(PhantomData<R>);

	impl<R> FrontierPrecompiles<R>
	where
		R: pallet_evm::Config,
	{
		pub fn new() -> Self {
			Self(Default::default())
		}
		pub fn used_addresses() -> sp_std::vec::Vec<H160> {
			sp_std::vec![1, 2, 3, 4, 5, 1024, 1025].into_iter().map(|x| hash(x)).collect()
		}
	}
	impl<R> PrecompileSet for FrontierPrecompiles<R>
	where
		R: pallet_evm::Config,
	{
		fn execute(
			&self,
			address: H160,
			input: &[u8],
			target_gas: Option<u64>,
			context: &Context,
			is_static: bool,
		) -> Option<PrecompileResult> {
			match address {
				// Ethereum precompiles :
				a if a == hash(1) =>
					Some(ECRecover::execute(input, target_gas, context, is_static)),
				a if a == hash(2) => Some(Sha256::execute(input, target_gas, context, is_static)),
				a if a == hash(3) =>
					Some(Ripemd160::execute(input, target_gas, context, is_static)),
				a if a == hash(5) => Some(Modexp::execute(input, target_gas, context, is_static)),
				a if a == hash(4) => Some(Identity::execute(input, target_gas, context, is_static)),
				a if a == hash(6) => Some(Bn128Add::execute(input, target_gas, context, is_static)),
				a if a == hash(7) => Some(Bn128Mul::execute(input, target_gas, context, is_static)),
				a if a == hash(8) =>
					Some(Bn128Pairing::execute(input, target_gas, context, is_static)),
				a if a == hash(9) => Some(Blake2F::execute(input, target_gas, context, is_static)),
				// Non-Frontier specific nor Ethereum precompiles :
				a if a == hash(1024) =>
					Some(Sha3FIPS256::execute(input, target_gas, context, is_static)),
				a if a == hash(1026) =>
					Some(ECRecoverPublicKey::execute(input, target_gas, context, is_static)),
				_ => None,
			}
		}

		fn is_precompile(&self, address: H160) -> bool {
			Self::used_addresses().contains(&address)
		}
	}

	fn hash(a: u64) -> H160 {
		H160::from_low_u64_be(a)
	}
}
