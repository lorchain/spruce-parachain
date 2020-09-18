#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	debug, decl_module, decl_storage, decl_error, decl_event, ensure, StorageValue, StorageMap, Parameter,
	traits::{Currency, ExistenceRequirement, Get},
	dispatch,
};
use sp_io::hashing::blake2_128;
use frame_system::{self as system, ensure_signed};
use sp_runtime::{
	ModuleId,
	traits::{
		StaticLookup, AccountIdConversion, Hash, One, Zero,
	}
};
use sp_std::{prelude::*, cmp, fmt::Debug, result};

mod join;

/// The pallet's configuration trait.
pub trait Trait: system::Trait + cdp::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type ModuleId: Get<ModuleId>;
}

// This pallet's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as ValleyModule {
		pub BeiTokenId get(fn bei_token_id): T::TokenId;
		pub ValTokenId get(fn val_token_id): T::TokenId;
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T> where
		AccountId = <T as frame_system::Trait>::AccountId,
		TokenId = <T as token::Trait>::TokenId,
		TokenBalance = <T as token::Trait>::TokenBalance,
	{
		BeiCreated(TokenId, AccountId),
		CollateralToBei(AccountId, AccountId, TokenId, TokenBalance, TokenBalance),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		NoneValue,
		StorageOverflow,
		InsufficientAmount,
		NotAllowed,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 10_000]
		pub fn create_bei(origin) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			let bei_token_id = token::Module::<T>::create_token(&sender, false, &[].to_vec());

			BeiTokenId::<T>::put(bei_token_id);

			Self::deposit_event(RawEvent::BeiCreated(bei_token_id, sender));
			Ok(())
		}

		#[weight = 10_000]
		pub fn collateral_to_bei(
			origin,
			account: T::AccountId,
			collateral_token: T::TokenId,
			amount: T::TokenBalance,
			target: T::TokenBalance,
		) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			join::collateral_join::<T>(collateral_token, sender.clone(), account.clone(), amount)?;
			cdp::Module::<T>::frob(collateral_token, account.clone(), amount, target);
			join::bei_exit::<T>(sender.clone(), account.clone(), target);

			Self::deposit_event(RawEvent::CollateralToBei(sender, account, collateral_token, amount, target));
			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
	}

}
