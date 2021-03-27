// Copyright 2017-2020 Parity Technologies (UK) Ltd.
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
//! Autogenerated weights for pallet_staking
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! DATE: 2021-03-24, STEPS: `[50, ]`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("indracore-dev"), DB CACHE: 128

// Executed Command:
// target/release/indracore
// benchmark
// --chain=indracore-dev
// --steps=50
// --repeat=20
// --pallet=pallet_staking
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --heap-pages=4096
// --header=./file_header.txt
// --output=./runtime/indracore/src/weights/

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for pallet_staking.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_staking::WeightInfo for WeightInfo<T> {
    fn bond() -> Weight {
        (75_102_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(5 as Weight))
            .saturating_add(T::DbWeight::get().writes(4 as Weight))
    }
    fn bond_extra() -> Weight {
        (57_637_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(3 as Weight))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
    fn unbond() -> Weight {
        (52_115_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(4 as Weight))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
    }
    fn withdraw_unbonded_update(s: u32) -> Weight {
        (53_109_000 as Weight)
            // Standard Error: 0
            .saturating_add((27_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(4 as Weight))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
    }
    fn withdraw_unbonded_kill(s: u32) -> Weight {
        (84_010_000 as Weight)
            // Standard Error: 1_000
            .saturating_add((2_603_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(6 as Weight))
            .saturating_add(T::DbWeight::get().writes(8 as Weight))
            .saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(s as Weight)))
    }
    fn validate() -> Weight {
        (14_760_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
    fn kick(k: u32) -> Weight {
        (10_438_000 as Weight)
            // Standard Error: 8_000
            .saturating_add((18_078_000 as Weight).saturating_mul(k as Weight))
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(k as Weight)))
            .saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(k as Weight)))
    }
    fn nominate(n: u32) -> Weight {
        (24_264_000 as Weight)
            // Standard Error: 13_000
            .saturating_add((5_606_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(T::DbWeight::get().reads(3 as Weight))
            .saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(n as Weight)))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
    fn chill() -> Weight {
        (14_023_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
    fn set_payee() -> Weight {
        (11_982_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn set_controller() -> Weight {
        (26_456_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(3 as Weight))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
    }
    fn set_validator_count() -> Weight {
        (1_981_000 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn force_no_eras() -> Weight {
        (2_320_000 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn force_new_era() -> Weight {
        (2_306_000 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn force_new_era_always() -> Weight {
        (2_244_000 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn set_invulnerables(v: u32) -> Weight {
        (2_368_000 as Weight)
            // Standard Error: 0
            .saturating_add((35_000 as Weight).saturating_mul(v as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn force_unstake(s: u32) -> Weight {
        (59_256_000 as Weight)
            // Standard Error: 1_000
            .saturating_add((2_584_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(4 as Weight))
            .saturating_add(T::DbWeight::get().writes(8 as Weight))
            .saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(s as Weight)))
    }
    fn cancel_deferred_slash(s: u32) -> Weight {
        (5_916_511_000 as Weight)
            // Standard Error: 389_000
            .saturating_add((34_617_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn payout_stakers_dead_controller(n: u32) -> Weight {
        (119_481_000 as Weight)
            // Standard Error: 15_000
            .saturating_add((50_212_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(T::DbWeight::get().reads(10 as Weight))
            .saturating_add(T::DbWeight::get().reads((3 as Weight).saturating_mul(n as Weight)))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
            .saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(n as Weight)))
    }
    fn payout_stakers_alive_staked(n: u32) -> Weight {
        (137_589_000 as Weight)
            // Standard Error: 20_000
            .saturating_add((64_957_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(T::DbWeight::get().reads(11 as Weight))
            .saturating_add(T::DbWeight::get().reads((5 as Weight).saturating_mul(n as Weight)))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
            .saturating_add(T::DbWeight::get().writes((3 as Weight).saturating_mul(n as Weight)))
    }
    fn rebond(l: u32) -> Weight {
        (34_803_000 as Weight)
            // Standard Error: 1_000
            .saturating_add((85_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(T::DbWeight::get().reads(3 as Weight))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
    }
    fn set_history_depth(e: u32) -> Weight {
        (0 as Weight)
            // Standard Error: 63_000
            .saturating_add((31_023_000 as Weight).saturating_mul(e as Weight))
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().writes(4 as Weight))
            .saturating_add(T::DbWeight::get().writes((7 as Weight).saturating_mul(e as Weight)))
    }
    fn reap_stash(s: u32) -> Weight {
        (63_152_000 as Weight)
            // Standard Error: 0
            .saturating_add((2_590_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(4 as Weight))
            .saturating_add(T::DbWeight::get().writes(8 as Weight))
            .saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(s as Weight)))
    }
    fn new_era(v: u32, n: u32) -> Weight {
        (0 as Weight)
            // Standard Error: 787_000
            .saturating_add((369_726_000 as Weight).saturating_mul(v as Weight))
            // Standard Error: 39_000
            .saturating_add((59_818_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(T::DbWeight::get().reads(10 as Weight))
            .saturating_add(T::DbWeight::get().reads((3 as Weight).saturating_mul(v as Weight)))
            .saturating_add(T::DbWeight::get().reads((3 as Weight).saturating_mul(n as Weight)))
            .saturating_add(T::DbWeight::get().writes(9 as Weight))
            .saturating_add(T::DbWeight::get().writes((3 as Weight).saturating_mul(v as Weight)))
    }
    fn get_npos_voters(v: u32, n: u32, s: u32) -> Weight {
        (0 as Weight)
            // Standard Error: 78_000
            .saturating_add((24_893_000 as Weight).saturating_mul(v as Weight))
            // Standard Error: 78_000
            .saturating_add((28_043_000 as Weight).saturating_mul(n as Weight))
            // Standard Error: 1_065_000
            .saturating_add((22_967_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(3 as Weight))
            .saturating_add(T::DbWeight::get().reads((3 as Weight).saturating_mul(v as Weight)))
            .saturating_add(T::DbWeight::get().reads((3 as Weight).saturating_mul(n as Weight)))
            .saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(s as Weight)))
    }
    fn get_npos_targets(v: u32) -> Weight {
        (0 as Weight)
            // Standard Error: 26_000
            .saturating_add((9_862_000 as Weight).saturating_mul(v as Weight))
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(v as Weight)))
    }
}
