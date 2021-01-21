#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure,
	StorageValue, StorageMap,
};
use frame_system::ensure_signed;
use sp_runtime::{
	traits::{One, Zero},
	DispatchError, DispatchResult, RuntimeDebug,
};
use sp_std::vec::Vec;
use primitives::{CurrencyId};


/// Currency info
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct CurrencyInfo<AccountId, TokenId> {
	/// Class owner
	pub creator: AccountId,
	/// Token id
	pub token: TokenId,
	/// Total issuance for the class
	pub total_supply: u128,
}

/// The pallet's configuration trait.
pub trait Trait: frame_system::Trait + token::Trait {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

pub type CurrencyInfoOf<T> =
	CurrencyInfo<<T as frame_system::Trait>::AccountId, <T as token::Trait>::TokenId>;

// This pallet's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as TemplateModule {
		pub Currencies get(fn currencies): map hasher(twox_64_concat) CurrencyId => Option<CurrencyInfoOf<T>>;
		pub NextCurrencyId get(fn next_currency_id): CurrencyId;
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T> where
		AccountId = <T as frame_system::Trait>::AccountId,
		CurrencyId = CurrencyId,
		TokenBalance = <T as token::Trait>::TokenBalance,
	{
		Created(CurrencyId, AccountId),
		Mint(CurrencyId, TokenBalance, AccountId),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		NoneValue,
		InvalidCurrencyId,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 0]
		pub fn create(origin, token_uri: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let currency_id = Self::next_currency_id();
	
			let token_id = token::Module::<T>::create_token(&who, true, &token_uri)?;
	
			let new_currency_info = CurrencyInfo {
				creator: who.clone(),
				token: token_id,
				total_supply: Default::default()
			};

			Currencies::<T>::insert(currency_id, new_currency_info);
			NextCurrencyId::mutate(|id| *id += <u64 as One>::one());

			Self::deposit_event(RawEvent::Created(currency_id, who));
			Ok(())
		}

		#[weight = 0]
		pub fn mint(origin, currency_id: CurrencyId, amount: T::TokenBalance, to: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let currency = Self::currencies(currency_id).ok_or(Error::<T>::InvalidCurrencyId)?;

			token::Module::<T>::mint(&to, &currency.token, amount)?;

			Self::deposit_event(RawEvent::Mint(currency_id, amount, to));

			Ok(())
		}

	}
}

impl<T: Trait> Module<T> {
	pub fn get_currency_token(currency_id: &CurrencyId) -> Result<T::TokenId, DispatchError> {
		let currency = Self::currencies(currency_id).ok_or(Error::<T>::InvalidCurrencyId)?;
		Ok(currency.token)
	}

	pub fn do_transfer_from(
		from: &T::AccountId,
		to: &T::AccountId,
		currency_id: &CurrencyId,
		amount: T::TokenBalance,
	) -> DispatchResult {
		let currency = Self::currencies(currency_id).ok_or(Error::<T>::InvalidCurrencyId)?;

		token::Module::<T>::transfer_from(from, to, &currency.token, amount)?;

		Ok(())
	}
}
