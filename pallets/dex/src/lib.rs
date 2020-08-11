#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	debug, decl_module, decl_storage, decl_error, decl_event, ensure, StorageValue, StorageMap, Parameter,
	traits::{Randomness, Currency, ExistenceRequirement, Get},
	dispatch,
};
use sp_io::hashing::blake2_128;
use frame_system::{self as system, ensure_signed};
use sp_runtime::{ModuleId,
	traits::{
		StaticLookup, AccountIdConversion, AtLeast32Bit, MaybeSerializeDeserialize,
		MaybeDisplay, Bounded, Member, SimpleBitOps, CheckEqual, MaybeSerialize,
		MaybeMallocSizeOf, Hash, One, Zero,
	}
};
use sp_std::{prelude::*, cmp, fmt::Debug, result};


/// The pallet's configuration trait.
pub trait Trait: system::Trait + pallet_timestamp::Trait + token::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type ModuleId: Get<ModuleId>;
	type PairId: Parameter + Member + AtLeast32Bit + Default + Copy
		+ MaybeSerializeDeserialize;

}

// const MINIMUM_LIQUIDITY: <T as token::Trait>::TokenBalance = 1000.into(); // 10**3
const MINIMUM_LIQUIDITY: u32 = 1000; // 10**3;

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Pair<T> where
	T: Trait
{
	token_a: T::TokenId,
	token_b: T::TokenId,
	pair_token: T::TokenId,
	account: T::AccountId,
}

// This pallet's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as DexModule {
		pub Pairs get(fn pairs): map hasher(blake2_128_concat) T::PairId => Option<Pair<T>>;
		pub NextPairId get(fn next_pair_id): T::PairId;
		pub OwnedPairs get(fn owned_pairs): map hasher(blake2_128_concat) T::AccountId => T::PairId;
		
		pub Reserves get(fn reserves): map hasher(blake2_128_concat) T::PairId => (T::TokenBalance, T::TokenBalance, T::BlockNumber);
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T> where 
		AccountId = <T as system::Trait>::AccountId,
		PairId = <T as Trait>::PairId,
	{
		PairCreated(AccountId, PairId),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		Overflow,
		InvalidPairId,
		InsufficientAmount,
		InsufficientOutAmount,
		InsufficientInputAmount,
		InsufficientOutputAmount,
		InsufficientLiquidity,
		AdjustedError,
		InsufficientAAmount,
		InsufficientBAmount,
		InsufficientLiquidityMinted,
		InsufficientLiquidityBurned,
		InvalidPath,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 0]
		pub fn create_pair(origin, token_a: T::TokenId, token_b: T::TokenId) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			let pair_id = NextPairId::<T>::get();

			let (token_0, token_1) = Self::sort_tokens(token_a, token_b);

			let pair_token = token::Module::<T>::create_token(&sender, false, [].to_vec());

			let account: T::AccountId = T::ModuleId::get().into_sub_account(&pair_token);

			let new_pair = Pair::<T> {
				token_a: token_0,
				token_b: token_1,
				pair_token,
				account: account.clone(),
			};

			Pairs::<T>::insert(pair_id, new_pair);
			NextPairId::<T>::mutate(|id| *id += One::one());

			Self::deposit_event(RawEvent::PairCreated(sender, pair_id));

			Ok(())
		}

		#[weight = 0]
		pub fn add_liquidity(
			origin,
			pair_id: T::PairId,
			token_a: T::TokenId,
			token_b: T::TokenId,
			amount_a_desired: T::TokenBalance,
			amount_b_desired: T::TokenBalance,
			amount_a_min: T::TokenBalance,
			amount_b_min: T::TokenBalance,
			to: T::AccountId,
			deadline: T::BlockNumber
		) -> dispatch::DispatchResult
		{
			let sender = ensure_signed(origin)?;

			let (amount_a, amount_b) = Self::do_add_liquidity(
				pair_id,
				token_a,
				token_b,
				amount_a_desired,
				amount_b_desired,
				amount_a_min,
				amount_b_min
			)?;

			let pair = Self::pairs(pair_id).ok_or(Error::<T>::InvalidPairId)?;

			token::Module::<T>::do_safe_transfer_from(sender.clone(), pair.account.clone(), token_a, amount_a);
			token::Module::<T>::do_safe_transfer_from(sender.clone(), pair.account.clone(), token_b, amount_b);
			let liquidity = Self::mint(pair_id, to)?;

			Ok(())
		}

		#[weight = 0]
		pub fn remove_liquidity(
			origin,
			pair_id: T::PairId,
			token_a: T::TokenId,
			token_b: T::TokenId,
			liquidity: T::TokenBalance,
			amount_a_min: T::TokenBalance,
			amount_b_min: T::TokenBalance,
			to: T::AccountId,
			deadline: T::BlockNumber
		) -> dispatch::DispatchResult
		{
			let sender = ensure_signed(origin)?;

			let pair = Self::pairs(pair_id).ok_or(Error::<T>::InvalidPairId)?;

			token::Module::<T>::do_safe_transfer_from(sender.clone(), pair.account.clone(), pair.pair_token, liquidity);
			let (amount_0, amount_1) = Self::burn(pair_id, to)?;
			let (token_0, token_1) = Self::sort_tokens(token_a, token_b);
			let (amount_a, amount_b) = if token_a == token_0 { (amount_0, amount_1) } else { (amount_1, amount_0) };

			ensure!(amount_a >= amount_a_min, Error::<T>::InsufficientAAmount);
			ensure!(amount_b >= amount_b_min, Error::<T>::InsufficientBAmount);

			Ok(())
		}

		#[weight = 0]
		pub fn swap_exact_tokens_for_tokens(
			origin,
			pair_id: T::PairId,
			amount_in: T::TokenBalance,
			amount_out_min: T::TokenBalance,
			path: Vec<T::TokenId>,
			to: T::AccountId,
			deadline: T::BlockNumber
		) -> dispatch::DispatchResult
		{
			let sender = ensure_signed(origin)?;

			let pair = Self::pairs(pair_id).ok_or(Error::<T>::InvalidPairId)?;

			let amounts = Self::get_amounts_out(pair_id, amount_in, &path)?;
			ensure!(amounts[amounts.len() - 1] >= amount_out_min, Error::<T>::InsufficientOutAmount);

			token::Module::<T>::do_safe_transfer_from(sender.clone(), pair.account.clone(), path[0], amounts[0]);
			Self::do_swap(pair_id, amounts, path, to);

			Ok(())
		}

		#[weight = 0]
		pub fn swap_tokens_for_exact_tokens(
			origin,
			pair_id: T::PairId,
			amount_out: T::TokenBalance,
			amount_in_max: T::TokenBalance,
			path: Vec<T::TokenId>,
			to: T::AccountId,
			deadline: T::BlockNumber
		) -> dispatch::DispatchResult
		{
			let sender = ensure_signed(origin)?;

			let pair = Self::pairs(pair_id).ok_or(Error::<T>::InvalidPairId)?;

			let amounts = Self::get_amounts_out(pair_id, amount_out, &path)?;
			ensure!(amounts[0] <= amount_in_max, Error::<T>::InsufficientInputAmount);

			token::Module::<T>::do_safe_transfer_from(sender.clone(), pair.account.clone(), path[0], amounts[0]);
			Self::do_swap(pair_id, amounts, path, to);

			Ok(())
		}

	}
}

impl<T: Trait> Module<T> {
	fn random_account(sender: &T::AccountId) -> T::AccountId {
		let payload = (
			T::Randomness::random_seed(),
			&sender,
			<frame_system::Module<T>>::extrinsic_index(),
		);
		let hash = payload.using_encoded(T::Hashing::hash);

		T::ModuleId::get().into_sub_account(&hash)
	}

	fn init_amount_in(
		balance: T::TokenBalance,
		reserve: T::TokenBalance,
		amount_out: T::TokenBalance
	) -> T::TokenBalance
	{
		if balance > reserve {
			balance - (reserve - amount_out)
		} else {
			Zero::zero()
		}
	}

	fn do_update(
		pair_id: T::PairId,
		balance_a: T::TokenBalance,
		balance_b: T::TokenBalance,
		_reserve_a: T::TokenBalance,
		_reserve_b: T::TokenBalance
	) -> dispatch::DispatchResult
	{
		ensure!(balance_a < Zero::zero() && balance_b < Zero::zero(), Error::<T>::Overflow);

		let now = frame_system::Module::<T>::block_number();

		Reserves::<T>::mutate(pair_id, |reserve| *reserve = (balance_a, balance_b, now));

		Ok(())
	}

	pub fn swap(
		pair_id: T::PairId,
		amount_0_out: T::TokenBalance,
		amount_1_out: T::TokenBalance,
		to: T::AccountId
	) -> dispatch::DispatchResult
	{
		let pair = Self::pairs(pair_id).ok_or(Error::<T>::InvalidPairId)?;

		ensure!(amount_0_out > Zero::zero() || amount_1_out > Zero::zero(), Error::<T>::InsufficientOutAmount);

		let (reserve_0, reserve_1, _) = Reserves::<T>::get(pair_id);
		ensure!(amount_0_out < reserve_0 && amount_1_out < reserve_1, Error::<T>::InsufficientLiquidity);

		if amount_0_out > Zero::zero() {
			token::Module::<T>::do_safe_transfer_from(pair.account.clone(), to.clone(), pair.token_a, amount_0_out);
		}
		if amount_1_out > Zero::zero() {
			token::Module::<T>::do_safe_transfer_from(pair.account.clone(), to.clone(), pair.token_b, amount_1_out);
		}

		let balance_0 = token::Module::<T>::balance_of((pair.token_a, pair.account.clone()));
		let balance_1 = token::Module::<T>::balance_of((pair.token_b, pair.account.clone()));

		let amount_0_in = Self::init_amount_in(balance_0, reserve_0, amount_0_out);
		let amount_1_in = Self::init_amount_in(balance_1, reserve_1, amount_1_out);
		
		ensure!(amount_0_in > Zero::zero() || amount_1_in > Zero::zero(), Error::<T>::InsufficientInputAmount);

		let balance_0_adjusted = balance_0 * 1000.into() - (amount_0_in * 3.into());
		let balance_1_adjusted = balance_1 * 1000.into() - (amount_1_in * 3.into());
		ensure!(balance_0_adjusted * balance_1_adjusted >= reserve_0 * reserve_1 * 1000.into() * 1000.into(), Error::<T>::AdjustedError);

		Self::do_update(pair_id, balance_0, balance_1, reserve_0, reserve_1);

		Ok(())
	}

	pub fn mint(
		pair_id: T::PairId,
		to: T::AccountId
	) -> result::Result<T::TokenBalance, dispatch::DispatchError>
	{
		let pair = Self::pairs(pair_id).ok_or(Error::<T>::InvalidPairId)?;

		let (reserve_a, reserve_b, _) = Reserves::<T>::get(pair_id);

		let balance_a = token::Module::<T>::balance_of((pair.token_a, pair.account.clone()));
		let balance_b = token::Module::<T>::balance_of((pair.token_b, pair.account.clone()));

		let amount_a = balance_a - reserve_a;
		let amount_b = balance_b - reserve_b;

		let liquidity: T::TokenBalance;

		let total_supply = token::Module::<T>::total_supply(pair.pair_token);
		if total_supply == Zero::zero() {
			liquidity = ((amount_a * amount_b) * (amount_a * amount_b)) - MINIMUM_LIQUIDITY.into();
			token::Module::<T>::mint(T::AccountId::default(), pair.pair_token, MINIMUM_LIQUIDITY.into()); // permanently lock the first MINIMUM_LIQUIDITY tokens
		} else {
			liquidity = cmp::min(amount_a * total_supply / reserve_a, amount_b * total_supply / reserve_b);
		}
		ensure!(liquidity >= Zero::zero(), Error::<T>::InsufficientLiquidityMinted);
		token::Module::<T>::mint(to, pair.pair_token, liquidity);

		Self::do_update(pair_id, balance_a, balance_b, reserve_a, reserve_b);

		Ok(liquidity)
	}

	pub fn burn (
		pair_id: T::PairId,
		to: T::AccountId
	) -> result::Result<(T::TokenBalance, T::TokenBalance), dispatch::DispatchError>
	{
		let pair = Self::pairs(pair_id).ok_or(Error::<T>::InvalidPairId)?;

		let (reserve_a, reserve_b, _) = Reserves::<T>::get(pair_id);

		let mut balance_a = token::Module::<T>::balance_of((pair.token_a, pair.account.clone()));
		let mut balance_b = token::Module::<T>::balance_of((pair.token_b, pair.account.clone()));

		let liquidity = token::Module::<T>::balance_of((pair.pair_token, pair.account.clone()));

		let total_supply = token::Module::<T>::total_supply(pair.pair_token);
		
		let amount_a = liquidity * balance_a / total_supply;
		let amount_b = liquidity * balance_b / total_supply;
		ensure!(amount_a > Zero::zero() && amount_b > Zero::zero(), Error::<T>::InsufficientLiquidityBurned);

		token::Module::<T>::burn(pair.account.clone(), pair.pair_token, liquidity);
		token::Module::<T>::do_safe_transfer_from(pair.account.clone(), to.clone(), pair.token_a, amount_a);
		token::Module::<T>::do_safe_transfer_from(pair.account.clone(), to.clone(), pair.token_b, amount_b);

		balance_a = token::Module::<T>::balance_of((pair.token_a, pair.account.clone()));
		balance_b = token::Module::<T>::balance_of((pair.token_b, pair.account.clone()));

		Self::do_update(pair_id, balance_a, balance_b, reserve_a, reserve_b);

		Ok((amount_a, amount_b))
	}

	fn do_swap(
		pair_id: T::PairId,
		amounts: Vec<T::TokenBalance>,
		path: Vec<T::TokenId>,
		_to: T::AccountId
	) -> dispatch::DispatchResult
	{
		let pair = Self::pairs(pair_id).ok_or(Error::<T>::InvalidPairId)?;

		for i in 0..path.len() - 1 {
			let (input, output) = (path[i], path[i + 1]);
			let (token_0, _) = Self::sort_tokens(input, output);
			let amount_out = amounts[i + 1];
			let (amount_0_out, amount_1_out) = if input == token_0 {
				(T::TokenBalance::from(0), amount_out)
			} else {
				(amount_out, T::TokenBalance::from(0))
			};
			let to = if i < path.len() - 2 { pair.account.clone() } else { _to.clone() };

			Self::swap(pair_id, amount_0_out, amount_1_out, to);
		}

		Ok(())
	}

	fn do_add_liquidity(
		pair_id: T::PairId,
		token_a: T::TokenId,
		token_b: T::TokenId,
		amount_a_desired: T::TokenBalance,
		amount_b_desired: T::TokenBalance,
		amount_a_min: T::TokenBalance,
		amount_b_min: T::TokenBalance
	) -> result::Result<(T::TokenBalance, T::TokenBalance), dispatch::DispatchError>
	{
		let pair = Self::pairs(pair_id);

		let (reserve_a, reserve_b, _) = Reserves::<T>::get(pair_id);

		let mut amount_a;
		let mut amount_b;
		if reserve_a == Zero::zero() && reserve_b == Zero::zero() {
			// (amount_a, amount_b) = (amount_a_desired, amount_b_desired);
			amount_a = amount_a_desired;
			amount_b = amount_b_desired;
		} else {
			let amount_b_optimal = Self::quote(amount_a_desired, reserve_a, reserve_b)?;
			if amount_b_optimal <= amount_b_desired {
				ensure!(amount_b_optimal >= amount_b_min, Error::<T>::InsufficientBAmount);
				// (amount_a, amount_b) = (amount_a_desired, amount_b_optimal);
				amount_a = amount_a_desired;
				amount_b = amount_b_optimal;
			} else {
				let amount_a_optimal = Self::quote(amount_b_desired, reserve_b, reserve_a)?;
				ensure!(amount_a_optimal <= amount_a_desired, Error::<T>::InsufficientAmount);
				ensure!(amount_a_optimal >= amount_a_min, Error::<T>::InsufficientAAmount);
				// (amount_a, amount_b) = (amount_a_optimal, amount_b_desired);
				amount_a = amount_a_optimal;
				amount_b = amount_b_desired;
			}
		}

		Ok((amount_a, amount_b))
	}

	fn sort_tokens(token_a: T::TokenId, token_b: T::TokenId) -> (T::TokenId, T::TokenId) {
		if token_a < token_b {
			(token_a, token_b)
		} else {
			(token_b, token_a)
		}
	}

	fn get_reserves(pair_id: T::PairId, token_a: T::TokenId, token_b: T::TokenId) -> (T::TokenBalance, T::TokenBalance) {
		let (token_0, _) = Self::sort_tokens(token_a, token_b);
		let (reserve_0, reserve_1, _) = Reserves::<T>::get(pair_id);
		let (reserve_a, reserve_b) = if token_a == token_0 {
			(reserve_0, reserve_1)
		} else {
			(reserve_1, reserve_0)
		};

		(reserve_a, reserve_b)
	}

	fn quote(
		amount_a: T::TokenBalance,
		reserve_a: T::TokenBalance,
		reserve_b: T::TokenBalance
	) -> result::Result<T::TokenBalance, dispatch::DispatchError>
	{
		ensure!(amount_a > Zero::zero(), Error::<T>::InsufficientAmount);
		ensure!(reserve_a > Zero::zero() && reserve_b > Zero::zero(), Error::<T>::InsufficientLiquidity);
		let amount_b = amount_a * reserve_b / reserve_a;

		Ok(amount_b)
	}

	fn get_amount_out(
		amount_in: T::TokenBalance,
		reserve_in: T::TokenBalance,
		reserve_out: T::TokenBalance
	) -> result::Result<T::TokenBalance, dispatch::DispatchError> {
		ensure!(amount_in > Zero::zero(), Error::<T>::InsufficientInputAmount);
		ensure!(reserve_in > Zero::zero() && reserve_out > Zero::zero(), Error::<T>::InsufficientLiquidity);

		let amount_in_with_fee = amount_in * 997.into();
		let numerator = amount_in_with_fee * reserve_out;
		let denominator = reserve_in * 1000.into() + amount_in_with_fee;
		let amount_out = numerator / denominator;

		Ok(amount_out)
	}

	fn get_amount_in(
		amount_out: T::TokenBalance,
		reserve_in: T::TokenBalance,
		reserve_out: T::TokenBalance
	) -> result::Result<T::TokenBalance, dispatch::DispatchError> {
		ensure!(amount_out > Zero::zero(), Error::<T>::InsufficientOutputAmount);
		ensure!(reserve_in > Zero::zero() && reserve_out > Zero::zero(), Error::<T>::InsufficientLiquidity);

		let numerator = reserve_in * amount_out * 1000.into();
		let denominator = (reserve_out - amount_out) * 997.into();
		let amount_in = (numerator / denominator) + 1.into();

		Ok(amount_in)
	}

	fn get_amounts_out(
		pair_id: T::PairId,
		amount_in: T::TokenBalance,
		path: &Vec<T::TokenId>
	) -> result::Result<Vec<T::TokenBalance>, dispatch::DispatchError> {
		ensure!(path.len() >= 2, Error::<T>::InvalidPath);

		let mut amounts = vec![T::TokenBalance::from(0); path.len()];
		amounts[0] = amount_in;
		for i in 0..path.len() - 1 {
			let (reserve_in, reserve_out) = Self::get_reserves(pair_id, path[i], path[i + 1]);
			amounts[i + 1] = Self::get_amount_out(amounts[i], reserve_in, reserve_out)?;
		}

		Ok(amounts)
	}

	fn get_amounts_in(
		pair_id: T::PairId,
		amount_out: T::TokenBalance,
		path: Vec<T::TokenId>
	) -> result::Result<Vec<T::TokenBalance>, dispatch::DispatchError> {
		ensure!(path.len() >= 2, Error::<T>::InvalidPath);

		let mut amounts = vec![T::TokenBalance::from(0); path.len()];
		amounts[path.len() - 1] = amount_out;
		for i in (0..path.len() - 1).rev() {
			let (reserve_in, reserve_out) = Self::get_reserves(pair_id, path[i - 1], path[i]);
			amounts[i - 1] = Self::get_amount_in(amounts[i], reserve_in, reserve_out)?;
		}

		Ok(amounts)
	}

}
