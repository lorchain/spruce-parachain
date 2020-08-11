#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	decl_module, decl_storage, decl_event, decl_error, ensure, dispatch,
	traits::{Currency, Get, ExistenceRequirement},
};
use frame_system::{self as system, ensure_signed};
use sp_runtime::{
	ModuleId,
	traits::{
		StaticLookup, AccountIdConversion, AtLeast32Bit, MaybeSerializeDeserialize,
		MaybeDisplay, Bounded, Member, SimpleBitOps, CheckEqual, MaybeSerialize,
		MaybeMallocSizeOf, Hash, One, Zero, Saturating, SaturatedConversion,
	}
};
use primitives::CurrencyId;
// use currencies::{MultiCurrency, MultiCurrencyExtended};

/// The pallet's configuration trait.
pub trait Trait: frame_system::Trait + token::Trait {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	type ModuleId: Get<ModuleId>;
	// type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;
	type Currency: Currency<Self::AccountId>;
}

type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

// This pallet's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as WrapModule {
		pub Balances get(fn balances): map hasher(blake2_128_concat) (CurrencyId, T::AccountId) => BalanceOf<T>;
		pub TotalSupply get(fn total_supply): map hasher(blake2_128_concat) CurrencyId => BalanceOf<T>;

		pub WrappedTokens get(fn wrapped_tokens): map hasher(blake2_128_concat) CurrencyId => T::TokenId;
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T> where 
		AccountId = <T as frame_system::Trait>::AccountId,
		TokenId = <T as token::Trait>::TokenId,
		Balance = BalanceOf<T>
	{
		Created(AccountId, CurrencyId, TokenId),
		Deposit(AccountId, CurrencyId, Balance),
		Withdrawal(AccountId, CurrencyId, Balance),
		Transfered(AccountId, AccountId, CurrencyId, Balance),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		NoneValue,
		StorageOverflow,
		AlreadyCreated,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 10_000]
		pub fn create(origin, currency_id: CurrencyId) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			ensure!(!WrappedTokens::<T>::contains_key(currency_id), Error::<T>::AlreadyCreated);

			let token_id = token::Module::<T>::create_token(&sender, false, [].to_vec());

			WrappedTokens::<T>::insert(currency_id, token_id);

			Self::deposit_event(RawEvent::Created(sender, currency_id, token_id));
			Ok(())
		}

		#[weight = 10_000]
		pub fn deposit(origin, currency_id: CurrencyId, value: BalanceOf<T>) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			let token_id = Self::wrapped_tokens(currency_id);

			let new_balance = Self::balances((currency_id, sender.clone())).saturating_add(value);

			T::Currency::transfer(&sender, &Self::account_id(), value, ExistenceRequirement::AllowDeath)?;
			token::Module::<T>::mint(sender.clone(), token_id, Self::convert(value));

			Balances::<T>::insert((currency_id, sender.clone()), new_balance);
			TotalSupply::<T>::mutate(currency_id, |bal| *bal += value);

			Self::deposit_event(RawEvent::Deposit(sender, currency_id, value));
			Ok(())
		}

		#[weight = 10_000]
		pub fn withdraw(origin, currency_id: CurrencyId, value: BalanceOf<T>) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			let token_id = Self::wrapped_tokens(currency_id);

			let new_balance = Self::balances((currency_id, sender.clone())).saturating_sub(value);

			T::Currency::transfer(&Self::account_id(), &sender, value, ExistenceRequirement::AllowDeath)?;
			token::Module::<T>::burn(sender.clone(), token_id, Self::convert(value));

			Balances::<T>::insert((currency_id, sender.clone()), new_balance);
			TotalSupply::<T>::mutate(currency_id, |bal| *bal -= value);

			Self::deposit_event(RawEvent::Withdrawal(sender, currency_id, value));

			Ok(())
		}

		#[weight = 10_000]
		pub fn transfer(origin, to: T::AccountId, currency_id: CurrencyId, value: BalanceOf<T>) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			Self::transfer_from(sender.clone(), to.clone(), currency_id, value);

			Self::deposit_event(RawEvent::Transfered(sender, to, currency_id, value));

			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
	}

	// pub fn convert_balance(value: BalanceOf<T>) -> T::TokenBalance {
	// 	// let value: u128 = amount.into();
	// 	value.into()
	// }

	fn convert(balance_of: BalanceOf<T>) -> T::TokenBalance {
		let value = balance_of.saturated_into::<u128>();
		value.saturated_into()
	}

	fn unconvert(token_balance: T::TokenBalance) -> BalanceOf<T> {
		let value = token_balance.saturated_into::<u128>();
		value.saturated_into()
	}


	pub fn transfer_from(from: T::AccountId, to: T::AccountId, currency_id: CurrencyId, value: BalanceOf<T>) -> dispatch::DispatchResult {
		// let new_from_balance = Self::balance_of((currency_id, from.clone()))
		// 	.checked_sub(value)
		// 	.ok_or(Error::<T>::StorageOverflow)?;

		// let new_to_balance = Self::balance_of((currency_id, to.clone()))
		// 	.checked_add(value)
		// 	.ok_or(Error::<T>::StorageOverflow)?;

		let new_from_balance = Self::balances((currency_id, from.clone())).saturating_add(value);
		let new_to_balance = Self::balances((currency_id, from.clone())).saturating_sub(value);

		Balances::<T>::insert((currency_id, from), new_from_balance);
		Balances::<T>::insert((currency_id, to), new_to_balance);

		Ok(())
	}
}
