// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Selendra.

// Selendra is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Selendra is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Selendra.  If not, see <http://www.gnu.org/licenses/>.

//! Generate reference performance check results.

use selendra_performance_test::PerfCheckError;

fn main() -> Result<(), PerfCheckError> {
	#[cfg(build_type = "release")]
	{
		run::run()
	}
	#[cfg(not(build_type = "release"))]
	{
		Err(PerfCheckError::WrongBuildType)
	}
}

#[cfg(build_type = "release")]
mod run {
	use selendra_node_core_pvf::sp_maybe_compressed_blob;
	use selendra_node_primitives::VALIDATION_CODE_BOMB_LIMIT;
	use selendra_performance_test::{
		measure_erasure_coding, measure_pvf_prepare, PerfCheckError, ERASURE_CODING_N_VALIDATORS,
	};
	use std::{
		fs::OpenOptions,
		io::{self, Write},
		time::Duration,
	};

	const WARM_UP_RUNS: usize = 16;
	const FILE_HEADER: &str = include_str!("../../../../file_header.txt");
	const DOC_COMMENT: &str = "//! This file was automatically generated by `gen-ref-constants`.\n//! Do not edit manually!";
	const FILE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/constants.rs");

	fn save_constants(pvf_prepare: Duration, erasure_coding: Duration) -> io::Result<()> {
		let mut output =
			OpenOptions::new().truncate(true).create(true).write(true).open(FILE_PATH)?;

		writeln!(output, "{}\n\n{}\n", FILE_HEADER, DOC_COMMENT)?;

		let pvf_prepare_millis = pvf_prepare.as_millis() as u64;
		let erasure_coding_millis = erasure_coding.as_millis() as u64;

		let token_stream = quote::quote! {
			use std::time::Duration;

			pub const PVF_PREPARE_TIME_LIMIT: Duration = Duration::from_millis(#pvf_prepare_millis);
			pub const ERASURE_CODING_TIME_LIMIT: Duration = Duration::from_millis(#erasure_coding_millis);
		};

		writeln!(output, "{}", token_stream.to_string())?;
		Ok(())
	}

	pub fn run() -> Result<(), PerfCheckError> {
		let _ = env_logger::builder().filter(None, log::LevelFilter::Info).try_init();

		let wasm_code =
			selendra_performance_test::WASM_BINARY.ok_or(PerfCheckError::WasmBinaryMissing)?;

		log::info!("Running the benchmark, number of iterations: {}", WARM_UP_RUNS);

		let code = sp_maybe_compressed_blob::decompress(wasm_code, VALIDATION_CODE_BOMB_LIMIT)
			.or(Err(PerfCheckError::CodeDecompressionFailed))?;

		let (pvf_prepare_time, erasure_coding_time) = (1..=WARM_UP_RUNS)
			.map(|i| {
				if i - 1 > 0 && (i - 1) % 5 == 0 {
					log::info!("{} iterations done", i - 1);
				}
				(
					measure_pvf_prepare(code.as_ref()),
					measure_erasure_coding(ERASURE_CODING_N_VALIDATORS, code.as_ref()),
				)
			})
			.last()
			.expect("`WARM_UP_RUNS` is greater than 1 and thus we have at least one element; qed");

		save_constants(pvf_prepare_time?, erasure_coding_time?)?;

		log::info!("Successfully stored new reference values at {:?}. Make sure to format the file via `cargo +nightly fmt`", FILE_PATH);

		Ok(())
	}
}
