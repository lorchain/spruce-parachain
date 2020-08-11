#![cfg_attr(not(feature = "std"), no_std)]

use primitives::{Balance, CurrencyId};
use codec::{Decode, Encode};
use frame_support::{
	decl_module, decl_storage, decl_event, decl_error, dispatch, ensure,
	traits::{Currency, Get},
};
use frame_system::{self as system, ensure_signed};

/// The pallet's configuration trait.
pub trait Trait: system::Trait + token::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Ilk<T> where
	T: Trait
{
	art: T::TokenBalance, // Total Normalised Debt
	rate: T::TokenBalance, // Accumulated Rates
	spot: T::TokenBalance, // Price with Safety Margin
	line: T::TokenBalance, // Debt Ceiling 
	dust: T::TokenBalance, // Urn Debt Floor
}

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Urn<T> where
	T: Trait
{
	ink: T::TokenBalance, // Locked Collateral
	art: T::TokenBalance, // Normalised Debt
}

// This pallet's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as CdpModule {
		pub Ilks get(fn ilks): map hasher(blake2_128_concat) T::TokenId => Option<Ilk<T>>;
		pub Urns get(fn urns): map hasher(blake2_128_concat) (T::TokenId, T::AccountId) => Option<Urn<T>>;
		pub Collateral get(fn collateral): map hasher(blake2_128_concat) (T::TokenId, T::AccountId) => T::TokenBalance;
		/// stablecoin
		pub Bei get(fn bei): map hasher(blake2_128_concat) T::AccountId => T::TokenBalance;
		/// Total Dai Issued 
		pub Debt get(fn debt): T::TokenBalance;
		/// System live flag
		pub Paused get(fn paused): bool;
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T> where 
		AccountId = <T as frame_system::Trait>::AccountId,
		TokenId = <T as token::Trait>::TokenId,
	{
		SetIlk(AccountId, TokenId),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		NoneValue,
		NotAllowed,
		InvalidCurrencyId,
		CollateralOverflow,
		CollateralNotEnough,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 10_000 + T::DbWeight::get().writes(1)]
		pub fn set_ilk(
			origin,
			token_id: T::TokenId,
			art: T::TokenBalance,
			rate: T::TokenBalance,
			spot: T::TokenBalance,
			line: T::TokenBalance,
			dust: T::TokenBalance,
		) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			let new_ilk = Ilk {
				art,
				rate,
				spot,
				line,
				dust,
			};

			Ilks::<T>::insert(token_id, new_ilk);

			Self::deposit_event(RawEvent::SetIlk(sender, token_id));
			Ok(())
		}

	}
}

impl<T: Trait> Module<T> {
	pub fn increase_collateral(token_id: T::TokenId, account: T::AccountId, amount: T::TokenBalance) -> dispatch::DispatchResult {
		// let new_collateral = Self::collateral((token_id, account.clone()))
		// 	.checked_add(amount)
		// 	.ok_or(Error::<T>::CollateralOverflow)?;
		let new_collateral = Self::collateral((token_id, account.clone())) + amount;
		
		Collateral::<T>::insert((token_id, account), new_collateral);
	
		Ok(())
	}

	pub fn decrease_collateral(token_id: T::TokenId, account: T::AccountId, amount: T::TokenBalance) -> dispatch::DispatchResult {
		// let new_collateral = Self::collateral((token_id, account.clone()))
		// 	.checked_sub(amount)
		// 	.ok_or(Error::<T>::CollateralNotEnough)?;
		let new_collateral = Self::collateral((token_id, account.clone())) - amount;
		
		Collateral::<T>::insert((token_id, account), new_collateral);
	
		Ok(())
	}

	pub fn transfer_collateral(token_id: T::TokenId, from: T::AccountId, to: T::AccountId, amount: T::TokenBalance) -> dispatch::DispatchResult {
		Collateral::<T>::mutate((token_id, from), |bal| *bal -= amount);
		Collateral::<T>::mutate((token_id, to), |bal| *bal += amount);
	
		Ok(())
	}
	
	pub fn transfer_bei(from: T::AccountId, to: T::AccountId, amount: T::TokenBalance) -> dispatch::DispatchResult {
		Bei::<T>::mutate(from, |bal| *bal -= amount);
		Bei::<T>::mutate(to, |bal| *bal += amount);
	
		Ok(())
	}
	
	pub fn frob(token_id: T::TokenId, account: T::AccountId, dink: T::TokenBalance, dart: T::TokenBalance) -> dispatch::DispatchResult {

		let mut new_ilk = Self::ilks(token_id).ok_or(Error::<T>::InvalidCurrencyId)?;
		let mut new_urn = Self::urns((token_id, account.clone())).ok_or(Error::<T>::InvalidCurrencyId)?;
		let debt = Self::debt();
	
		new_urn.ink += dink;
		new_urn.art += dart;
	
		new_ilk.art += dart;
	
		let dtab = new_ilk.rate * dart;
		// let tab = ilk.rate * urn.art;
		let new_debt = debt + dtab;
	
		Collateral::<T>::mutate((token_id, account.clone()), |bal| *bal -= dink);
		Bei::<T>::mutate(account.clone(), |bal| *bal += dtab);
	
		Debt::<T>::put(new_debt);
		Ilks::<T>::insert(token_id, new_ilk);
		Urns::<T>::insert((token_id, account), new_urn);

		Ok(())
	}
}
