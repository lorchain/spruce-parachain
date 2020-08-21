#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure, StorageValue, StorageMap, Parameter,
	dispatch,
};
use frame_system::{self as system, ensure_signed};
use sp_runtime::{traits::{AtLeast32Bit, Bounded, Member, One}};
use sp_std::prelude::*;


pub trait Trait: frame_system::Trait + token::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type TaoId: Parameter + Member + AtLeast32Bit + Bounded + Default + Copy;
}

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub struct Tao<T> where
	T: Trait
{
	creator: T::AccountId,
	uri: Vec<u8>,
}

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub struct Token<T> where
	T: Trait
{
	pub tao: T::TaoId,
	pub is_commodity: bool,
}

decl_storage! {
	trait Store for Module<T: Trait> as TaoModule {
		pub Taos get(fn taos): map hasher(blake2_128_concat) T::TaoId => Option<Tao<T>>;

		pub NextTaoId get(fn next_tao_id): T::TaoId;

		pub OwnedTaos get(fn owned_taos): map hasher(blake2_128_concat) (T::AccountId, T::Index) => T::TaoId;
		pub OwnedTaoCount get(fn owned_tao_count): T::Index;
		pub OwnedTaoIndex get(fn owned_tao_index): map hasher(blake2_128_concat) T::TaoId => T::Index;

		pub TaoTokens get(fn tao_tokens): map hasher(blake2_128_concat) (T::TaoId, T::Index) => T::TokenId;
		pub TaoTokenCount get(fn tao_token_count): T::Index;
		pub TaoTokenIndex get(fn tao_token_index): map hasher(blake2_128_concat) T::TokenId => T::Index;

		pub OwnedTaoTokens get(fn owned_tao_tokens): map hasher(blake2_128_concat) (T::AccountId, T::TaoId) => T::TokenId;

		pub TaoOfToken get(fn tao_of_token): map hasher(blake2_128_concat) T::TokenId => T::TaoId;
		pub IsCommodity get(fn is_commodity): map hasher(blake2_128_concat) T::TokenId => bool;

		pub Tokens get(fn tokens): map hasher(blake2_128_concat) T::TokenId => Option<Token<T>>;

	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		InvalidTaoId,
		RequireOwner,
		InvalidProduct,
		InvalidTokenId,
	}
}

decl_event!(
	pub enum Event<T> where
		<T as frame_system::Trait>::AccountId,
		<T as Trait>::TaoId,
		<T as token::Trait>::TokenId,
		<T as token::Trait>::TokenBalance,
	{
		CreateTao(AccountId, TaoId),
		CreateTaoToken(AccountId, TaoId, TokenId),
		Mint(AccountId, AccountId, TaoId, TokenId, TokenBalance),
		Burn(AccountId, AccountId, TaoId, TokenId, TokenBalance),
	}
);

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 0]
		pub fn create_tao(origin, uri: Vec<u8>) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			let tao_id = NextTaoId::<T>::get();

			let tao = Tao {
				creator: sender.clone(),
				uri: uri.clone(),
			};

			Taos::<T>::insert(tao_id, tao);
			NextTaoId::<T>::mutate(|id| *id += <T::TaoId as One>::one());

			Self::deposit_event(RawEvent::CreateTao(sender, tao_id));

			Ok(())
		}

		#[weight = 0]
		pub fn create_tao_token(origin, tao_id: T::TaoId, is_nf: bool, uri: Vec<u8>, is_commodity: bool) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			ensure!(Taos::<T>::contains_key(tao_id), Error::<T>::InvalidTaoId);

			let token_id = token::Module::<T>::create_token(&sender, is_nf, uri);

			let token_index = Self::tao_token_count();

			TaoTokens::<T>::insert((tao_id, token_index), token_id);
			TaoTokenCount::<T>::mutate(|count| *count += One::one());
			TaoTokenIndex::<T>::insert(token_id, token_index);

			let new_token = Token::<T> {
				tao: tao_id,
				is_commodity,
			};
			Tokens::<T>::insert(token_id, new_token);

			Self::deposit_event(RawEvent::CreateTaoToken(sender, tao_id, token_id));

			Ok(())
		}

		#[weight = 0]
		pub fn mint(origin, to: T::AccountId, token_id: T::TokenId, amount: T::TokenBalance) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			let token = Self::tokens(token_id).unwrap();
			ensure!(token.is_commodity == false, Error::<T>::InvalidTokenId);

			Self::do_mint(token.tao, token_id, to.clone(), amount);

			Self::deposit_event(RawEvent::Mint(sender, to, token.tao, token_id, amount));

			Ok(())
		}

		#[weight = 0]
		pub fn burn(origin, to: T::AccountId, token_id: T::TokenId, amount: T::TokenBalance) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			let token = Self::tokens(token_id).unwrap();
			ensure!(token.is_commodity == false, Error::<T>::InvalidTokenId);

			Self::do_burn(token.tao, token_id, to.clone(), amount);

			Self::deposit_event(RawEvent::Burn(sender, to, token.tao, token_id, amount));

			Ok(())
		}

	}
}

impl<T: Trait> Module<T> {
	fn insert_owned_tao_token(tao_id: T::TaoId, token_id: T::TokenId) {
		let token_index = Self::tao_token_count();

		TaoTokens::<T>::insert((tao_id, token_index), token_id);
		TaoTokenCount::<T>::mutate(|count| *count += One::one());
		TaoTokenIndex::<T>::insert(token_id, token_index);
	}

	fn remove_owned_tao_token(tao_id: T::TaoId, token_id: T::TokenId) {
		let token_index = Self::tao_token_count();

		TaoTokens::<T>::remove((tao_id, token_index));
		TaoTokenCount::<T>::mutate(|count| *count -= One::one());
		TaoTokenIndex::<T>::remove(token_id);
	}

	pub fn do_mint(tao_id: T::TaoId, token_id: T::TokenId, account: T::AccountId, amount: T::TokenBalance) -> dispatch::DispatchResult {
		// ensure!(TaoOfToken::<T>::contains_key(token_id), Error::<T>::InvalidTokenId);
		// let tao_id = Self::tao_of_token(token_id);
		
		token::Module::<T>::mint(token_id, &account, amount)?;

		Self::insert_owned_tao_token(tao_id, token_id);


		Ok(())
	}

	pub fn do_burn(tao_id: T::TaoId, token_id: T::TokenId, account: T::AccountId, amount: T::TokenBalance) -> dispatch::DispatchResult {
		// ensure!(TaoOfToken::<T>::contains_key(token_id), Error::<T>::InvalidTokenId);
		// let tao_id = Self::tao_of_token(token_id);

		token::Module::<T>::burn(token_id, &account, amount)?;

		Self::remove_owned_tao_token(tao_id, token_id);

		Ok(())
	}

}
