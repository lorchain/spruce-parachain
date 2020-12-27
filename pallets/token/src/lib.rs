#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure, StorageValue, StorageMap, Parameter,
	dispatch::{DispatchResult, DispatchError},
};
use sp_runtime::{
	traits::{
		AtLeast32Bit, MaybeSerializeDeserialize, Bounded, Member,
		One, AtLeast32BitUnsigned, CheckedAdd, CheckedSub,
	},
	RuntimeDebug,
};
// use sp_std::fmt::Debug;
// use sp_std::prelude::*;
use sp_std::vec::Vec;
use sp_std::vec;

/// The module's configuration trait.
pub trait Trait: frame_system::Trait {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	
	type TokenBalance: Parameter + Member + AtLeast32BitUnsigned + Default + Copy +
		MaybeSerializeDeserialize + From<u128> + Into<u128>;
    
    type TokenId: Parameter + Member + AtLeast32Bit + Bounded + Default + Copy;
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct Token<AccountId> {
	creator: AccountId,
	is_nf: bool,
	uri: Vec<u8>,
}

decl_storage! {
    trait Store for Module<T: Trait> as TokenModule {
		pub Tokens get(fn tokens): map hasher(blake2_128_concat) T::TokenId => Option<Token<T::AccountId>>;
		pub TokenCount get(fn token_count): u64;
		pub NextTokenId get(fn next_token_id): T::TokenId;

		pub Balances get(fn balances):
			double_map hasher(twox_64_concat) T::TokenId, hasher(twox_64_concat) T::AccountId => T::TokenBalance;
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		InvalidTokenId,
		InsufficientBalance,
		NumOverflow,
		InvalidArrayLength,
	}
}

decl_event!(
    pub enum Event<T> where
		AccountId = <T as frame_system::Trait>::AccountId,
		TokenId = <T as Trait>::TokenId,
		TokenBalance = <T as Trait>::TokenBalance,
    {
		Created(TokenId, AccountId),
		Mint(AccountId, TokenId, TokenBalance),
		BatchMint(AccountId, Vec<TokenId>, Vec<TokenBalance>),
		Burn(AccountId, TokenId, TokenBalance),
		BatchBurn(AccountId, Vec<TokenId>, Vec<TokenBalance>),
		Transferred(AccountId, AccountId, TokenId, TokenBalance),
		BatchTransferred(AccountId, AccountId, Vec<TokenId>, Vec<TokenBalance>),
	}
);

decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;
	}
}

impl<T: Trait> Module<T> {
	pub fn create_token(who: &T::AccountId, is_nf: bool, uri: &Vec<u8>) -> Result<T::TokenId, DispatchError> {
		let token_id = Self::next_token_id();

		let new_token = Token {
			creator: who.clone(),
			is_nf,
			uri: uri.clone(),
		};

		Tokens::<T>::insert(token_id, new_token);
		TokenCount::mutate(|count| *count += <u64 as One>::one());
		NextTokenId::<T>::mutate(|id| *id += One::one());

		Self::deposit_event(RawEvent::Created(token_id.clone(), who.clone()));

		Ok(token_id)
	}

	pub fn mint(
		to: &T::AccountId,
		id: &T::TokenId,
		amount: T::TokenBalance
	) -> DispatchResult {
		Balances::<T>::try_mutate(id, to, |balance| -> DispatchResult {
			*balance = balance
				.checked_add(&amount)
				.ok_or(Error::<T>::NumOverflow)?;
			Ok(())
		})?;

		Self::deposit_event(RawEvent::Mint(to.clone(), id.clone(), amount));

		Ok(())
	}

	pub fn batch_mint(
		to: &T::AccountId,
		ids: &Vec<T::TokenId>,
		amounts: Vec<T::TokenBalance>
	) -> DispatchResult {
		ensure!(ids.len() == amounts.len(), Error::<T>::InvalidArrayLength);

		let n = ids.len();

		for i in 0..n {
			let id = ids[i];
			let amount = amounts[i];

			Balances::<T>::try_mutate(id, to, |balance| -> DispatchResult {
				*balance = balance
					.checked_add(&amount)
					.ok_or(Error::<T>::NumOverflow)?;
				Ok(())
			})?;
		}

		Self::deposit_event(RawEvent::BatchMint(to.clone(), ids.clone(), amounts));

		Ok(())
	}

	pub fn burn(
		from: &T::AccountId,
		id: &T::TokenId,
		amount: T::TokenBalance
	) -> DispatchResult {
		Balances::<T>::try_mutate(id, from, |balance| -> DispatchResult {
			*balance = balance
				.checked_sub(&amount)
				.ok_or(Error::<T>::NumOverflow)?;
			Ok(())
		})?;

		Self::deposit_event(RawEvent::Burn(from.clone(), id.clone(), amount));

		Ok(())
	}

	pub fn batch_burn(
		from: &T::AccountId,
		ids: &Vec<T::TokenId>,
		amounts: Vec<T::TokenBalance>
	) -> DispatchResult {
		ensure!(ids.len() == amounts.len(), Error::<T>::InvalidArrayLength);

		let n = ids.len();

		for i in 0..n {
			let id = ids[i];
			let amount = amounts[i];

			Balances::<T>::try_mutate(id, from, |balance| -> DispatchResult {
				*balance = balance
					.checked_sub(&amount)
					.ok_or(Error::<T>::NumOverflow)?;
				Ok(())
			})?;
		}

		Self::deposit_event(RawEvent::BatchBurn(from.clone(), ids.clone(), amounts));

		Ok(())
	}

	pub fn transfer_from(
		from: &T::AccountId,
		to: &T::AccountId,
		id: &T::TokenId,
		amount: T::TokenBalance
	) -> DispatchResult {
		if from == to {
			return Ok(());
		}

		Balances::<T>::try_mutate(id, from, |balance| -> DispatchResult {
			*balance = balance
				.checked_sub(&amount)
				.ok_or(Error::<T>::NumOverflow)?;
			Ok(())
		})?;

		Balances::<T>::try_mutate(id, to, |balance| -> DispatchResult {
			*balance = balance
				.checked_add(&amount)
				.ok_or(Error::<T>::NumOverflow)?;
			Ok(())
		})?;

		Self::deposit_event(RawEvent::Transferred(from.clone(), to.clone(), id.clone(), amount));

		Ok(())
	}

	pub fn batch_transfer_from(
		from: &T::AccountId,
		to: &T::AccountId,
		ids: &Vec<T::TokenId>,
		amounts: Vec<T::TokenBalance>
	) -> DispatchResult {
		if from == to {
			return Ok(());
		}

		ensure!(ids.len() == amounts.len(), Error::<T>::InvalidArrayLength);

		let n = ids.len();

		for i in 0..n {
			let id = &ids[i];
			let amount = amounts[i];

			Balances::<T>::try_mutate(id, from, |balance| -> DispatchResult {
				*balance = balance
					.checked_sub(&amount)
					.ok_or(Error::<T>::NumOverflow)?;
				Ok(())
			})?;
	
			Balances::<T>::try_mutate(id, to, |balance| -> DispatchResult {
				*balance = balance
					.checked_add(&amount)
					.ok_or(Error::<T>::NumOverflow)?;
				Ok(())
			})?;
		}

		Self::deposit_event(RawEvent::BatchTransferred(from.clone(), to.clone(), ids.to_vec(), amounts));

		Ok(())
	}

	pub fn balance_of(owner: &T::AccountId, id: &T::TokenId) -> T::TokenBalance {
		Self::balances(id, owner)
	}

	pub fn balance_of_batch(owners: &Vec<T::AccountId>, ids: &Vec<T::TokenId>) -> Result<Vec<T::TokenBalance>, DispatchError> {
		ensure!(owners.len() == ids.len(), Error::<T>::InvalidArrayLength);

		let mut batch_balances = vec![T::TokenBalance::from(0); owners.len()];

		let n = owners.len();

		for i in 0..n {
			let owner = &owners[i];
			let id = ids[i];

			batch_balances[i] = Self::balances(id, owner);
		}

		Ok(batch_balances)
	}

}
