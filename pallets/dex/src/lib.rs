#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure, StorageValue, StorageMap, Parameter,
	traits::Get,
};
use frame_system::{self as system, ensure_signed};
use sp_runtime::{
	ModuleId,
	traits::{
		AccountIdConversion, One, Zero,
	},
	DispatchError, DispatchResult, RuntimeDebug,
};
use primitives::{BlockNumber, CurrencyId};
use sp_std::prelude::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;


pub type ExchangeId = u64;

/// The pallet's configuration trait.
pub trait Trait: system::Trait + pallet_timestamp::Trait + currency::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type ModuleId: Get<ModuleId>;
	// type PairId: Parameter + Member + AtLeast32Bit + Default + Copy
	// 	+ MaybeSerializeDeserialize;

}

/// Exchange info
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct ExchangeInfo<AccountId> {
	/// Class owner
	pub creator: AccountId,
	/// Token id
	pub currency: CurrencyId,
	pub vault: AccountId,
}

// This pallet's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as DexModule {
		pub Exchanges get(fn exchanges): map hasher(blake2_128_concat) ExchangeId => Option<ExchangeInfo<T::AccountId>>;
		pub NextExchangeId get(fn next_exchange_id): ExchangeId;

		pub TotalSupplies get(fn total_supplies): map hasher(blake2_128_concat) T::TokenId => T::TokenBalance;
		pub CurrencyReserves get(fn currency_reserves): map hasher(blake2_128_concat) T::TokenId => T::TokenBalance;
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T> where
		AccountId = <T as system::Trait>::AccountId,
		TokenId = <T as token::Trait>::TokenId,
		TokenBalance = <T as token::Trait>::TokenBalance,
	{
		ExchangeCreated(ExchangeId, AccountId),
		CurrencyToToken(ExchangeId, AccountId, AccountId, Vec<TokenId>, Vec<TokenBalance>, Vec<TokenBalance>),
		TokenToCurrency(ExchangeId, AccountId, AccountId, Vec<TokenId>, Vec<TokenBalance>, Vec<TokenBalance>),
		LiquidityAdded(AccountId, AccountId, Vec<TokenId>, Vec<TokenBalance>, Vec<TokenBalance>),
		LiquidityRemoved(AccountId, AccountId, Vec<TokenId>, Vec<TokenBalance>, Vec<TokenBalance>),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		Overflow,
		InvalidExchangeId,
		InvalidMaxCurrency,
		InsufficientCurrencyAmount,
		InsufficientTokenAmount,
		SameCurrencyAndToken,
		MaxCurrencyAmountExceeded,
		InvalidCurrencyAmount,
		InsufficientLiquidity,
		InsufficientOutputAmount,
		InsufficientInputAmount,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 0]
		pub fn create_exchange(origin, currency_id: CurrencyId) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let exchange_id = Self::next_exchange_id();

			let account = T::ModuleId::get().into_sub_account(exchange_id);

			let new_exchange_info = ExchangeInfo {
				creator: sender.clone(),
				currency: currency_id,
				vault: account,
			};

			Exchanges::<T>::insert(exchange_id, new_exchange_info);
			NextExchangeId::mutate(|id| *id += <ExchangeId as One>::one());

			Self::deposit_event(RawEvent::ExchangeCreated(exchange_id, sender));

			Ok(())
		}

		#[weight = 0]
		pub fn currency_to_token(
			origin,
			exchange_id: ExchangeId,
			token_ids: Vec<T::TokenId>,
			token_amounts_out: Vec<T::TokenBalance>,
			max_currency: T::TokenBalance,
			to: T::AccountId,
			deadline: BlockNumber,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let exchange = Self::exchanges(exchange_id).ok_or(Error::<T>::InvalidExchangeId)?;

			let n = token_ids.len();

			let mut total_refund_currency: T::TokenBalance = max_currency;

			let mut amounts_in = vec![T::TokenBalance::from(0u32); n];
			// let mut token_reserves = vec![0 as T::TokenBalance; n];

			let token_reserves = Self::get_token_reserves(&exchange.vault, &token_ids);

			for i in 0..n {
				let id = token_ids[i];
				let amount_out = token_amounts_out[i];
				let token_reserve = token_reserves[i];

				let currency_reserve = Self::currency_reserves(id);
				let currency_amount = Self::get_amount_in(amount_out, currency_reserve, token_reserve)?;

				total_refund_currency -= currency_amount;

				amounts_in[i] = currency_amount;

				CurrencyReserves::<T>::mutate(id, |currency_reserve| *currency_reserve += currency_amount);
			}

			if total_refund_currency > Zero::zero()  {
				currency::Module::<T>::do_transfer_from(&exchange.vault, &to, &exchange.currency, total_refund_currency)?;
			}

			token::Module::<T>::batch_transfer_from(&exchange.vault, &to, &token_ids, token_amounts_out.clone())?;

			Self::deposit_event(RawEvent::CurrencyToToken(exchange_id, sender, to, token_ids, token_amounts_out, amounts_in));

			Ok(())
		}

		#[weight = 0]
		pub fn token_to_currency(
			origin,
			exchange_id: ExchangeId,
			token_ids: Vec<T::TokenId>,
			token_amounts_in: Vec<T::TokenBalance>,
			min_currency: T::TokenBalance,
			to: T::AccountId,
			deadline: BlockNumber,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let exchange = Self::exchanges(exchange_id).ok_or(Error::<T>::InvalidExchangeId)?;

			let n = token_ids.len();
			let mut total_currency = T::TokenBalance::from(0u32);
			let mut amounts_out = vec![T::TokenBalance::from(0u32); n];
			// let mut token_reserves = vec![0 as T::TokenBalance; n];

			let token_reserves = Self::get_token_reserves(&exchange.vault, &token_ids);

			for i in 0..n {
				let id = token_ids[i];
				let amount_in = token_amounts_in[i];
				let token_reserve = token_reserves[i];

				let currency_reserve = Self::currency_reserves(id);
				let currency_amount = Self::get_amount_out(amount_in, token_reserve - amount_in, currency_reserve)?;

				total_currency += currency_amount;
				amounts_out[i] = currency_amount;

				CurrencyReserves::<T>::mutate(id, |currency_reserve| *currency_reserve -= currency_amount);
			}

			ensure!(total_currency >= min_currency, Error::<T>::InsufficientCurrencyAmount);

			currency::Module::<T>::do_transfer_from(&exchange.vault, &to, &exchange.currency, total_currency)?;

			Self::deposit_event(RawEvent::TokenToCurrency(exchange_id, sender, to, token_ids, token_amounts_in, amounts_out));

			Ok(())
		}

		#[weight = 0]
		pub fn add_liquidity(
			origin,
			exchange_id: ExchangeId,
			to: T::AccountId,
			token_ids: Vec<T::TokenId>,
			token_amounts: Vec<T::TokenBalance>,
			max_currencys: Vec<T::TokenBalance>,
			deadline: BlockNumber,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let exchange = Self::exchanges(exchange_id).ok_or(Error::<T>::InvalidExchangeId)?;
			let currency_token = currency::Module::<T>::get_currency_token(&exchange.currency)?;

			let n = token_ids.len();
			let mut total_currency = T::TokenBalance::from(0u32);

			let mut liquidities_to_mint = vec![T::TokenBalance::from(0u32); n];
			let mut currency_amounts = vec![T::TokenBalance::from(0u32); n];
			// let mut token_reserves = [0 as T::TokenBalance; n];

			let token_reserves = Self::get_token_reserves(&exchange.vault, &token_ids);

			for i in 0..n {
				let id = token_ids[i];
				let amount = token_amounts[i];

				ensure!(max_currencys[i] > Zero::zero() , Error::<T>::InvalidMaxCurrency);
				ensure!(amount > Zero::zero() , Error::<T>::InsufficientTokenAmount);

				ensure!(currency_token != id, Error::<T>::SameCurrencyAndToken);

				let total_liquidity = Self::total_supplies(id);

				if total_liquidity > Zero::zero()  {
					let currency_reserve = Self::currency_reserves(id);
					let token_reserve = token_reserves[i];

					let (currency_amount, rounded) = Self::div_round(amount * currency_reserve, token_reserve - amount);
					ensure!(max_currencys[i] >= currency_amount, Error::<T>::MaxCurrencyAmountExceeded);

					total_currency = total_currency + currency_amount;

					let fixed_currency_amount = if rounded { currency_amount - 1u32.into() } else { currency_amount };
					liquidities_to_mint[i] = (fixed_currency_amount * total_liquidity) / currency_reserve;
					currency_amounts[i] = currency_amount;

					CurrencyReserves::<T>::mutate(id, |currency_reserve| *currency_reserve += currency_amount);
					TotalSupplies::<T>::mutate(id, |total_supply| *total_supply = total_liquidity + liquidities_to_mint[i]);
				} else {
					let max_currency = max_currencys[i];
					// ensure!(max_currency >= 1000000000u32.into(), Error::<T>::InvalidCurrencyAmount);
					ensure!(max_currency >= 1000u32.into(), Error::<T>::InvalidCurrencyAmount);

					total_currency = total_currency + max_currency;
					liquidities_to_mint[i] = max_currency;
					currency_amounts[i] = max_currency;

					CurrencyReserves::<T>::mutate(id, |currency_reserve| *currency_reserve = max_currency);
					TotalSupplies::<T>::mutate(id, |total_supply| *total_supply = max_currency);
				}
			}

			token::Module::<T>::batch_mint(&to, &token_ids, liquidities_to_mint)?;

			currency::Module::<T>::do_transfer_from(&sender, &exchange.vault, &exchange.currency, total_currency)?;

			Self::deposit_event(RawEvent::LiquidityAdded(sender, to, token_ids, token_amounts, currency_amounts));

			Ok(())
		}

		#[weight = 0]
		pub fn remove_liquidity(
			origin,
			exchange_id: ExchangeId,
			to: T::AccountId,
			token_ids: Vec<T::TokenId>,
			liquidities: Vec<T::TokenBalance>,
			min_currencys: Vec<T::TokenBalance>,
			min_tokens: Vec<T::TokenBalance>,
			deadline: BlockNumber,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let exchange = Self::exchanges(exchange_id).ok_or(Error::<T>::InvalidExchangeId)?;

			let n = token_ids.len();
			let mut total_currency = T::TokenBalance::from(0u32);

			let mut token_amounts = vec![T::TokenBalance::from(0u32); n];
			let mut currency_amounts = vec![T::TokenBalance::from(0u32); n];
			// let mut token_reserves = [0 as T::TokenBalance; n];

			let token_reserves = Self::get_token_reserves(&exchange.vault, &token_ids);

			for i in 0..n {
				let id = token_ids[i];
				let liquidity = liquidities[i];
				let token_reserve = token_reserves[i];

				let total_liquidity = Self::total_supplies(id);
				ensure!(total_liquidity > Zero::zero() , Error::<T>::InsufficientLiquidity);

				let currency_reserve = Self::currency_reserves(id);

				let currency_amount = liquidity * currency_reserve / total_liquidity;
				let token_amount = liquidity * token_reserve / total_liquidity;

				ensure!(currency_amount >= min_currencys[i], Error::<T>::InsufficientCurrencyAmount);
				ensure!(token_amount >= min_tokens[i], Error::<T>::InsufficientTokenAmount);

				total_currency += currency_amount;
				token_amounts[i] = token_amount;
				currency_amounts[i] = currency_amount;

				CurrencyReserves::<T>::mutate(id, |currency_reserve| *currency_reserve -= currency_amount);
				TotalSupplies::<T>::mutate(id, |total_supply| *total_supply = total_liquidity - liquidity);
			}

			token::Module::<T>::batch_burn(&exchange.vault, &token_ids, liquidities)?;

			currency::Module::<T>::do_transfer_from(&exchange.vault, &to, &exchange.currency, total_currency)?;
			token::Module::<T>::batch_transfer_from(&exchange.vault, &to, &token_ids, token_amounts.clone())?;

			Self::deposit_event(RawEvent::LiquidityRemoved(sender, to, token_ids, token_amounts, currency_amounts));

			Ok(())
		}

	}
}

impl<T: Trait> Module<T> {
	fn get_amount_in(
		amount_out: T::TokenBalance,
		reserve_in: T::TokenBalance,
		reserve_out: T::TokenBalance,
	) -> Result<T::TokenBalance, DispatchError> {
		ensure!(amount_out > Zero::zero() , Error::<T>::InsufficientOutputAmount);
		ensure!(reserve_in > Zero::zero()  && reserve_out > Zero::zero() , Error::<T>::InsufficientLiquidity);

		let numerator = reserve_in * amount_out * 1000u32.into();
		let denominator = (reserve_out - amount_out) * 995u32.into();
		let (amount_in, _) = Self::div_round(numerator, denominator);

		Ok(amount_in)
	}

	fn get_amount_out(
		amount_in: T::TokenBalance,
		reserve_in: T::TokenBalance,
		reserve_out: T::TokenBalance,
	) -> Result<T::TokenBalance, DispatchError> {
		ensure!(amount_in > Zero::zero() , Error::<T>::InsufficientInputAmount);
		ensure!(reserve_in > Zero::zero()  && reserve_out > Zero::zero() , Error::<T>::InsufficientLiquidity);

		let amount_in_with_fee = amount_in * 995u32.into();
		let numerator = amount_in_with_fee * reserve_out;
		let denominator = (reserve_in * 1000u32.into()) + amount_in_with_fee;
		let amount_out = numerator / denominator;

		Ok(amount_out)
	}

	fn get_token_reserves(vault: &T::AccountId, token_ids: &Vec<T::TokenId>) -> Vec<T::TokenBalance> {
		let n = token_ids.len();

		if n == 1 {
			let mut token_reserves = vec![T::TokenBalance::from(0u32); n];
			token_reserves[0] = token::Module::<T>::balance_of(vault, &token_ids[0]);
			token_reserves
		} else {
			let vaults = vec![vault.clone(); n];
			let token_reserves = token::Module::<T>::balance_of_batch(&vaults, &token_ids).unwrap();
			token_reserves
		}
	}

	fn div_round(a: T::TokenBalance, b: T::TokenBalance) -> (T::TokenBalance, bool) {
		if a % b > Zero::zero() {
			(a / b, false)
		} else {
			((a / b) + 1u32.into(), true)
		}
	}
}
