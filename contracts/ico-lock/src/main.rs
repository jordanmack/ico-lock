#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(alloc_error_handler)]
#![feature(panic_info_message)]

// Import `Result` from `core` instead of from `std` since we are in no-std mode.
use core::result::Result;

// Import CKB syscalls and structures.
// https://nervosnetwork.github.io/ckb-std/riscv64imac-unknown-none-elf/doc/ckb_std/index.html
// use ckb_std::{debug, default_alloc, entry};
use ckb_std::{default_alloc, entry};
use ckb_std::ckb_constants::Source;
use ckb_std::ckb_types::{bytes::Bytes, packed::Bytes as Args, packed::Script, prelude::*};
use ckb_std::error::{SysError};
use ckb_std::high_level::{load_cell, load_cell_capacity, load_cell_data, load_cell_lock_hash, load_script, QueryIter};

const COST_AMOUNT_LEN: usize = 8; /// Number of bytes for the token cost amount. (u64)
const LOCK_HASH_LEN: usize = 32; /// Number of bytes for a lock hash.
const SUDT_AMOUNT_DATA_LEN: usize = 16; /// Number of bytes for an SUDT amount. (u128)
const ARGS_LEN: usize = LOCK_HASH_LEN + COST_AMOUNT_LEN; /// Number of bytes required for args.

entry!(entry);
default_alloc!();

/// Program entry point.
fn entry() -> i8
{
	// Call main function and return error code.
	match main()
	{
		Ok(_) => 0,
		Err(err) => err as i8,
	}
}

/// Local error values.
/// Low values are reserved for Sys Error codes.
/// Values 100+ are for custom errors.
#[repr(i8)]
enum Error
{
	IndexOutOfBound = 1,
	ItemMissing,
	LengthNotEnough,
	Encoding,
	ArgsLen = 100,
	AmountCkbytes,
	AmountSudt,
	ExchangeRate,
	InvalidCost,
	InvalidStructure,
}

/// Map Sys Errors to local Error values.
impl From<SysError> for Error
{
	fn from(err: SysError) -> Self
	{
		use SysError::*;
		match err
		{
			IndexOutOfBound => Self::IndexOutOfBound,
			ItemMissing => Self::ItemMissing,
			LengthNotEnough(_) => Self::LengthNotEnough,
			Encoding => Self::Encoding,
			Unknown(err_code) => panic!("Unexpected Sys Error: {}", err_code),
		}
	}
}

fn check_owner_mode(args: &Args) -> Result<bool, Error>
{
	// With owner lock script extracted, we will look through each input in the
	// current transaction to see if any unlocked cell uses owner lock.
	let args: Bytes = args.unpack();
	let is_owner_mode = QueryIter::new(load_cell_lock_hash, Source::Input)
		.find(|lock_hash| args[0..LOCK_HASH_LEN] == lock_hash[..]).is_some();

	Ok(is_owner_mode)
}

fn determine_ico_cell_amounts(lock_script: &Script, type_script: &Script, source: Source) -> Result<(u64, u128), Error>
{
	let mut buf = [0u8; SUDT_AMOUNT_DATA_LEN];
	let lock_script_bytes = lock_script.as_bytes();
	let type_script_bytes = type_script.as_bytes();

	let mut capacity = 0;
	let mut amount = 0;
	let mut i = 0;
	loop
	{
		let cell = match load_cell(i, source)
		{
			Ok(cell) => cell,
			Err(SysError::IndexOutOfBound) => break,
			Err(e) => return Err(e.into()),
		};

		let lock = cell.lock();
		let type_ = cell.type_().to_opt();
		if type_.is_none()
		{
			i += 1;
			continue;
		}

		// Check if this Cell matches the Lock Script and Type Script.
		if lock.as_bytes()[..] == lock_script_bytes[..] && type_.unwrap().as_bytes()[..] == type_script_bytes[..]
		{
			capacity += load_cell_capacity(i, source)?;

			let data = load_cell_data(i, source)?;
			if data.len() == SUDT_AMOUNT_DATA_LEN
			{
				buf.copy_from_slice(&data);
				amount += u128::from_le_bytes(buf);
			}
			else
			{
				return Err(Error::Encoding);
			}
		}

		i += 1;
	}

	Ok((capacity, amount))
}

fn determine_token_cost(args: &Args) -> Result<u64, Error>
{
	let args: Bytes = args.unpack();
	let mut buf = [0u8; COST_AMOUNT_LEN];

	let slice_start = LOCK_HASH_LEN;
	let slice_end = slice_start + COST_AMOUNT_LEN;

	buf.copy_from_slice(&args[slice_start..slice_end]);
	let token_cost = u64::from_le_bytes(buf);

	if token_cost < 1
	{
		return Err(Error::InvalidCost);
	}

	Ok(token_cost)
}

fn validate_amounts(token_cost: u64, input_capacity_amount: u64, output_capacity_amount: u64, input_token_amount: u128, output_token_amount: u128) -> Result<(), Error>
{
	if output_capacity_amount < input_capacity_amount
	{
		return Err(Error::AmountCkbytes);
	}

	if output_token_amount > input_token_amount
	{
		return Err(Error::AmountSudt);
	}

	if (output_capacity_amount - input_capacity_amount) as u128 != (input_token_amount - output_token_amount) * token_cost as u128
	{
		return Err(Error::ExchangeRate);
	}

	Ok(())
}

fn validate_ico_inputs() -> Result<(Script, Script), Error>
{
	// Verify that index 1 does not exist.
	if load_cell(1, Source::GroupInput).is_ok()
	{
		return Err(Error::InvalidStructure);
	}

	// Load the ico cell. There should be exactly 1.
	let ico_cell = load_cell(0, Source::GroupInput)?;

	// Extract the Scripts. Both must exist.
	let lock_script = ico_cell.lock();
	let type_script = ico_cell.type_().to_opt().ok_or(Error::InvalidStructure)?;

	Ok((lock_script, type_script))
}

fn validate_ico_outputs(lock_script: &Script, type_script: &Script) -> Result<(), Error>
{
	let mut i = 0;
	let mut ico_lock_cells = 0;
	let mut ico_lock_matching_type_cells = 0;
	loop
	{
		let cell = match load_cell(i, Source::Output)
		{
			Ok(cell) => cell,
			Err(SysError::IndexOutOfBound) => break,
			Err(e) => return Err(e.into()),
		};

		let cell_lock_bytes = &cell.lock().as_bytes()[..];
		let cell_type_bytes = &cell.type_().as_bytes()[..];
		let lock_script_bytes = &lock_script.as_bytes()[..];
		let type_script_bytes = &type_script.as_bytes()[..];

		if cell_lock_bytes == lock_script_bytes
		{
			ico_lock_cells += 1;

			if cell_type_bytes == type_script_bytes
			{
				ico_lock_matching_type_cells += 1;
			}
		}

		i += 1;
	}

	// debug!("Total ICO Lock Cells: {}", ico_lock_cells);
	// debug!("Total ICO Lock Cells w/ Matching Type Script: {}", ico_lock_matching_type_cells);

	// There must be exactly one output ICO Lock Cell and it must have a Type Script matching the input ICO Lock Cell.
	if ico_lock_cells != 1 || ico_lock_matching_type_cells != 1
	{
		return Err(Error::InvalidStructure);
	}

	Ok(())
}

fn main() -> Result<(), Error>
{
	// Load arguments from the current script.
	let script = load_script()?;
	let args = script.args();

	// Verify that the minimum length of the arguments was given.
	if args.len() < ARGS_LEN
	{
		return Err(Error::ArgsLen);
	}

	// If program is in owner mode then unlock immediately.
	if check_owner_mode(&args)?
	{
		// debug!("ICO Lock owner mode enabled.");
		return Ok(());
	}

	// Check the inputs to ensure there is a single input ICO Cell.
	let (lock_script, type_script) = validate_ico_inputs()?;

	// Check the outputs to ensure there is a single output ICO Cell.
	validate_ico_outputs(&lock_script, &type_script)?;

	// Find all the capacity, token, and cost amounts.
	let (input_capacity_amount, input_token_amount) = determine_ico_cell_amounts(&lock_script, &type_script, Source::GroupInput)?;
	let (output_capacity_amount, output_token_amount) = determine_ico_cell_amounts(&lock_script, &type_script, Source::Output)?;
	let token_cost = determine_token_cost(&args)?;

	// debug!("Input/Output Capacity: {}/{}", input_capacity_amount, output_capacity_amount);
	// debug!("Input/Output Token Amount: {}/{}", input_token_amount, output_token_amount);
	// debug!("Token Cost: {}", token_cost);

	// Validate that all amounts are in balance.
	validate_amounts(token_cost, input_capacity_amount, output_capacity_amount, input_token_amount, output_token_amount)?;

	// Args Definition
	// 0: Governance lock script hash (32 Bytes)
	// 1: Cost per token in CKByte Shannons. (u64 LE 8 Bytes)

	// Rules
	// 1. The arguments must be equal or greater than 40 bytes in length.
	// 2. If an input Cell's lock hash matches that specified in the args, owner mode is then enabled and the Cell unlocks unconditionally.
	// 3. There must be exactly one input Cell with the ICO Lock Script and exactly one output Cell with the ICO Lock Script.
	// 4. The Type Script of both the input ICO Cell and output ICO Cell must match.
	// 5. The capacity on the output ICO Cell must be equal or higher than on the input ICO Cell.
	// 6. The SUDT amount of the output ICO Cell must be equal or lower than the input ICO Cell.
	// 7. The cost of SUDTs in Shannons must be greater than or equal to 1.
	// 8. The capacity difference between the input/output ICO Cells divided by the cost must equal the SUDT amount difference between the input/output ICO Cells.

	Ok(())
}
