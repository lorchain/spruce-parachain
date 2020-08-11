#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure, StorageValue, StorageMap, Parameter,
	traits::{Randomness, Currency, ExistenceRequirement},
	dispatch,
};
use sp_io::hashing::blake2_128;
use frame_system::{self as system, ensure_signed};
use sp_runtime::{traits::{AtLeast32Bit, Bounded, Member, Hash, One}};
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

decl_storage! {
	trait Store for Module<T: Trait> as TaoModule {
		pub Taos get(fn taos): map hasher(blake2_128_concat) T::TaoId => Option<Tao<T>>;

		pub NextTaoId get(fn next_tao_id): T::TaoId;

		pub OwnedTaos get(fn owned_taos): map hasher(blake2_128_concat) (T::AccountId, T::Index) => T::TaoId;
		pub OwnedTaoCount get(fn owned_tao_count): T::Index;
		pub OwnedTaoIndex get(fn owned_tao_index): map hasher(blake2_128_concat) T::TaoId => T::Index;

		pub OwnedTaoTokens get(fn owned_tao_tokens): map hasher(blake2_128_concat) (T::TaoId, T::Index) => T::TokenId;
		pub OwnedTaoTokenCount get(fn owned_tao_token_count): T::Index;
		pub OwnedTaoTokenIndex get(fn owned_tao_token_index): map hasher(blake2_128_concat) T::TokenId => T::Index;
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		InvalidTaoId,
		RequireOwner,
	}
}

decl_event!(
	pub enum Event<T> where
		<T as frame_system::Trait>::AccountId,
		<T as Trait>::TaoId,
		<T as token::Trait>::TokenId,
	{
		CreateTao(AccountId, TaoId),
		CreateTaoToken(AccountId, TaoId, TokenId),
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
		pub fn create_tao_token(origin, tao_id: T::TaoId, is_nf: bool, uri: Vec<u8>) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			ensure!(Taos::<T>::contains_key(tao_id), Error::<T>::InvalidTaoId);

			let token_id = token::Module::<T>::create_token(&sender, is_nf, uri);

			let token_index = Self::owned_tao_token_count();

			OwnedTaoTokens::<T>::insert((tao_id, token_index), token_id);
			OwnedTaoTokenCount::<T>::mutate(|count| *count += One::one());
			OwnedTaoTokenIndex::<T>::insert(token_id, token_index);

			Self::deposit_event(RawEvent::CreateTaoToken(sender, tao_id, token_id));

			Ok(())
		}

		#[weight = 0]
		pub fn create_tao_token_with_collateral(origin, tao_id: T::TaoId, is_nf: bool, uri: Vec<u8>) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			Ok(())
		}

	}
}

impl<T: Trait> Module<T> {

}
