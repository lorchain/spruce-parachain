#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	debug, decl_module, decl_storage, decl_error, decl_event, ensure, StorageValue, StorageMap, Parameter,
	traits::Randomness,
	dispatch::{DispatchResult, DispatchError},
};
use sp_io::hashing::blake2_128;
use frame_system::{self as system, ensure_signed};
use sp_runtime::{traits::{
	AtLeast32Bit, MaybeSerializeDeserialize, MaybeDisplay, Bounded, Member,
	SimpleBitOps, MaybeSerialize, Hash, One, Saturating,
	Zero, AtLeast32BitUnsigned, CheckedAdd, CheckedSub,
}};
use sp_std::fmt::Debug;
use sp_std::prelude::*;
use sp_std::vec::Vec;

/// The module's configuration trait.
pub trait Trait: frame_system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	// type Currency: Currency<Self::AccountId>;
	type Randomness: Randomness<Self::Hash>;
	
	type TokenBalance: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + Debug +
		MaybeSerializeDeserialize + From<u128> + Into<u128>;
    
    type TokenId: Parameter + Member + Debug + Default + Copy + Ord
		+ MaybeSerializeDeserialize + From<[u8;32]> + Into<[u8;32]>;
}

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub struct Token<T> where
	T: Trait
{
	id: T::TokenId,
	creator: T::AccountId,
	is_nf: bool,
	uri: Vec<u8>,
}

decl_storage! {
    trait Store for Module<T: Trait> as TokenModule {
		pub Tokens get(fn tokens): map hasher(blake2_128_concat) T::TokenId => Option<Token<T>>;
		pub TokenCount get(fn token_count): u64;
		pub NFIndex get(fn nf_index): map hasher(blake2_128_concat) T::TokenId => u128;

		// ERC1155
		pub Balances get(fn balances):
			double_map hasher(twox_64_concat) T::TokenId, hasher(blake2_128_concat) T::AccountId => T::TokenBalance;
		pub Allowances get(fn allowances):
			double_map  hasher(twox_64_concat) T::TokenId, hasher(blake2_128_concat) (T::AccountId, T::AccountId) => T::TokenBalance;
		pub OperatorApproval get(fn operator_approval): map hasher(blake2_128_concat) (T::AccountId, T::AccountId) => bool;
		pub TotalSupply get(fn total_supply): map hasher(blake2_128_concat) T::TokenId => T::TokenBalance;
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		InvalidTokenId,
		InvalidNonFungibleId,
		RequireNonFungible,
		RequireFungible,
		LengthMismatch,
		InsufficientBalance,
		TransferSameAccount,
		TokenNotExists,
	}
}

decl_event!(
    pub enum Event<T> where
		AccountId = <T as frame_system::Trait>::AccountId,
		TokenId = <T as Trait>::TokenId,
		TokenBalance = <T as Trait>::TokenBalance,
    {
		Created(AccountId, TokenId),
		MintNonFungible(TokenId, AccountId, TokenId),
		MintFungible(TokenId, AccountId, TokenBalance),
		Burn(TokenId, AccountId, TokenBalance),
		BurnBatch(TokenId, Vec<AccountId>, Vec<TokenBalance>),
		Transferred(TokenId, AccountId, AccountId, TokenBalance),
        Approval(AccountId, AccountId, TokenBalance),
    }
);

decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 0]
        fn set_approval_for_all(origin, operator: T::AccountId, approved: bool) -> DispatchResult {
           let sender = ensure_signed(origin)?;

		   OperatorApproval::<T>::mutate((sender, operator), |approval| *approval = approved);

           Ok(())
        }

		#[weight = 0]
        fn safe_transfer_from(
			origin,
			to: T::AccountId,
			id: T::TokenId,
			amount: T::TokenBalance
		) -> DispatchResult 
		{
			let sender = ensure_signed(origin)?;

			Self::do_safe_transfer_from(&id, &sender, &to, amount)?;

			Ok(())
		}

		#[weight = 0]
        fn safe_batch_transfer_from(
			origin,
			to: T::AccountId,
			ids: Vec<T::TokenId>,
			amounts: Vec<T::TokenBalance>
		) -> DispatchResult 
		{
			let sender = ensure_signed(origin)?;

			for i in 0..ids.len() {
				let id = ids[i];
				let amount = amounts[i];

				Self::do_safe_transfer_from(&id, &sender, &to, amount)?;
			}

			Ok(())
		}

		#[weight = 0]
		fn debug_create_token(origin, is_nf: bool) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			debug::info!("run into crate token");

			let id = Self::create_token(&sender, is_nf, [].to_vec());
			// let id = Self::get_token_id(&sender);
			debug::info!("id is {:?}", id);

			Ok(())
		}

		// #[weight = 0]
		// fn debug_mint_nf(origin, token_id: T::TokenId, to: Vec<T::AccountId>) -> DispatchResult {
		// 	let sender = ensure_signed(origin)?;

		// 	debug::info!("run into mint nf");

		// 	Self::mint_non_fungible(token_id, &to)?;

		// 	Ok(())
		// }

		// #[weight = 0]
		// fn debug_mint_f(origin, token_id: T::TokenId, to: Vec<T::AccountId>, amounts: Vec<T::TokenBalance>) -> DispatchResult {
		// 	let sender = ensure_signed(origin)?;

		// 	debug::info!("run into mint nf");

		// 	Self::mint_fungible(token_id, &to, amounts)?;

		// 	Ok(())
		// }

    }
}

impl<T: Trait> Module<T> {
	fn random_type_id(sender: &T::AccountId) -> T::TokenId {
		let payload = (
			T::Randomness::random_seed(),
			&sender,
			<frame_system::Module<T>>::extrinsic_index(),
		);

		let random = payload.using_encoded(blake2_128);

		let mut array = [0; 32];
		array[..16].copy_from_slice(&random[..]);
		// debug::info!("array is {:?}", array);

		array.into()
	}

	fn get_token_id(type_id: &T::TokenId, index: u128) -> T::TokenId {
		// let id_bytes = type_id.into();
		let id_bytes = type_id.clone().into();
		let index_bytes = index.to_be_bytes();

		let mut array = [0 as u8; 32];
		array[..16].copy_from_slice(&id_bytes[..16]);
		array[16..].copy_from_slice(&index_bytes[..16]);

		array.into()
	}

	fn get_type_id(token_id: &T::TokenId) -> T::TokenId {
		// let id_bytes = token_id.into();
		let id_bytes = token_id.clone().into();

		let mut array = [0 as u8; 32];
		array[..16].copy_from_slice(&id_bytes[..16]);

		array.into()
	}

	pub fn is_non_fungible(token_id: &T::TokenId) -> bool {
		let type_id = Self::get_type_id(token_id);
		let token = Self::tokens(type_id).unwrap();
		token.is_nf
	}

	pub fn exists(token_id: &T::TokenId) -> bool {
		Self::tokens(token_id).is_some()
	}

	pub fn create_token(who: &T::AccountId, is_nf: bool, uri: Vec<u8>) -> T::TokenId {
		let type_id = Self::random_type_id(&who);

		let token = Token::<T> {
			id: type_id,
			creator: who.clone(),
			is_nf,
			uri: uri.clone(),
		};

		Tokens::<T>::insert(type_id, token);
		TokenCount::mutate(|id| *id += <u64 as One>::one());

		Self::deposit_event(RawEvent::Created(who.clone(), type_id));

		type_id
	}
	
	pub fn mint_non_fungible(
		token_id: &T::TokenId,
		accounts: &Vec<T::AccountId>,
	) -> Result<(), DispatchError> {
		ensure!(Self::is_non_fungible(token_id), Error::<T>::RequireNonFungible);

		let type_id = Self::get_type_id(token_id);
		ensure!(*token_id == type_id, Error::<T>::InvalidNonFungibleId);

		let index = Self::nf_index(type_id).checked_add(<u128 as One>::one()).expect("NF index error");
		NFIndex::<T>::mutate(type_id, |index| *index += accounts.len() as u128);

		for i in 0..accounts.len() {
			let to = &accounts[i];
			let amount = T::TokenBalance::from(1);
			let id = Self::get_token_id(&type_id, index + i as u128);

			Balances::<T>::mutate(type_id, to, |balance| *balance = balance.saturating_add(amount));
			TotalSupply::<T>::mutate(type_id, |supply| *supply = supply.saturating_add(amount));

			Self::deposit_event(RawEvent::MintNonFungible(type_id, to.clone(), id));
		}

		Ok(())
	}

	pub fn mint_fungible(
		token_id: &T::TokenId,
		accounts: &Vec<T::AccountId>,
		amounts: Vec<T::TokenBalance>,
	) -> Result<(), DispatchError> {
		ensure!(!Self::is_non_fungible(token_id), Error::<T>::RequireFungible);
		ensure!(accounts.len() == amounts.len(), Error::<T>::LengthMismatch);

		for i in 0..accounts.len() {
			let to = &accounts[i];
			let amount = amounts[i];

			Balances::<T>::mutate(token_id, to, |balance| *balance = balance.saturating_add(amount));
			TotalSupply::<T>::mutate(token_id, |supply| *supply = supply.saturating_add(amount));

			Self::deposit_event(RawEvent::MintFungible(*token_id, to.clone(), amount));
		}

		Ok(())
	}

	pub fn mint(token_id: &T::TokenId, account: &T::AccountId, amount: T::TokenBalance) -> Result<(), DispatchError>  {
		ensure!(Self::exists(token_id), Error::<T>::TokenNotExists);
		let is_nf = Self::is_non_fungible(token_id);

		if is_nf {
			Self::mint_non_fungible(token_id, &[ account.clone() ].to_vec());
		} else {
			Self::mint_fungible(token_id, &[ account.clone() ].to_vec(), [ amount ].to_vec());
		}

		Ok(())
	}

	pub fn mint_batch(token_id: &T::TokenId, accounts: &Vec<T::AccountId>, amounts: Vec<T::TokenBalance>) -> Result<(), DispatchError>  {
		ensure!(Self::exists(token_id), Error::<T>::TokenNotExists);
		let is_nf = Self::is_non_fungible(token_id);

		if is_nf {
			Self::mint_non_fungible(token_id, &accounts);
		} else {
			Self::mint_fungible(token_id, &accounts, amounts);
		}

		Ok(())
	}

	pub fn burn(token_id: &T::TokenId, account: &T::AccountId, amount: T::TokenBalance) -> Result<(), DispatchError>  {
		ensure!(Self::exists(token_id), Error::<T>::TokenNotExists);

		if Self::is_non_fungible(token_id) {
			let type_id = Self::get_type_id(token_id);

			Balances::<T>::mutate(type_id, account, |balance| *balance = balance.saturating_sub(amount));
			TotalSupply::<T>::mutate(type_id, |supply| *supply = supply.saturating_sub(amount));
		} else {
			Balances::<T>::mutate(token_id, account, |balance| *balance = balance.saturating_sub(amount));
			TotalSupply::<T>::mutate(token_id, |supply| *supply = supply.saturating_sub(amount));
		}

		Self::deposit_event(RawEvent::Burn(*token_id, account.clone(), amount));

		Ok(())
	}

	pub fn burn_batch(token_id: &T::TokenId, accounts: &Vec<T::AccountId>, amounts: Vec<T::TokenBalance>) -> Result<(), DispatchError>  {
		ensure!(Self::exists(token_id), Error::<T>::TokenNotExists);

		for i in 0..accounts.len() {
			let to = &accounts[i];
			let amount = amounts[i];

			if Self::is_non_fungible(token_id) {
				let type_id = Self::get_type_id(token_id);
	
				Balances::<T>::mutate(type_id, to, |balance| *balance = balance.saturating_sub(amount));
				TotalSupply::<T>::mutate(type_id, |supply| *supply = supply.saturating_sub(amount));
			} else {
				Balances::<T>::mutate(token_id, to, |balance| *balance = balance.saturating_sub(amount));
				TotalSupply::<T>::mutate(token_id, |supply| *supply = supply.saturating_sub(amount));
			}
		}

		Self::deposit_event(RawEvent::BurnBatch(*token_id, accounts.clone(), amounts));
		
		Ok(())
	}

	pub fn do_safe_transfer_from(
		token_id: &T::TokenId,
		from: &T::AccountId,
		to: &T::AccountId,
		amount: T::TokenBalance
	) -> DispatchResult {
		let _new_balance = Self::balances(token_id, from)
			.checked_sub(&amount)
			.ok_or(Error::<T>::InsufficientBalance)?;

		if from != to {
			if Self::is_non_fungible(token_id) {
				let type_id = Self::get_type_id(token_id);
	
				Balances::<T>::mutate(type_id, from, |balance| *balance -= amount);
				Balances::<T>::mutate(type_id, to, |balance| *balance += amount);
			} else {
				Balances::<T>::mutate(token_id, from, |balance| *balance -= amount);
				Balances::<T>::mutate(token_id, to, |balance| *balance += amount);
			}

			Self::deposit_event(RawEvent::Transferred(*token_id, from.clone(), to.clone(), amount));
		}

		Ok(())
	}

}
