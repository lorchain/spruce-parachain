#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure,
	StorageValue, StorageMap, Parameter,
	dispatch::DispatchResult,
};
use frame_system::ensure_signed;
use sp_runtime::{
	ModuleId, RuntimeDebug,
	traits::{
		AtLeast32Bit, Bounded, Member, One, CheckedAdd, CheckedSub,
	},
};
use sp_std::prelude::*;


pub type TaoItemId = u64;

pub trait Trait: frame_system::Trait + token::Trait {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	type TaoId: Parameter + Member + AtLeast32Bit + Bounded + Default + Copy;
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct Tao<AccountId> {
	pub creator: AccountId,
	pub properties: Vec<u8>,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct TaoItemInfo<AccountId, TokenId> {
	pub owner: AccountId,
	pub token: TokenId,
}

decl_storage! {
	trait Store for Module<T: Trait> as TaoModule {
		pub Taos get(fn taos): map hasher(blake2_128_concat) T::TaoId => Option<Tao<T::AccountId>>;
		pub NextTaoId get(fn next_tao_id): T::TaoId;

		pub NextTaoItemId get(fn next_tao_item_id): map hasher(blake2_128_concat) T::TaoId => TaoItemId;
		pub TaoItems get(fn tao_items): double_map hasher(twox_64_concat) T::TaoId, hasher(twox_64_concat) TaoItemId => Option<TaoItemInfo<T::AccountId, T::TokenId>>;
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		InvalidTaoId,
	}
}

decl_event!(
	pub enum Event<T> where
		AccountId = <T as frame_system::Trait>::AccountId,
		TaoId = <T as Trait>::TaoId,
	{
		TaoCreated(TaoId, AccountId),
		TaoItemCreated(TaoId, TaoItemId, AccountId),
	}
);

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 0]
		pub fn create_tao(origin, properties: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let tao_id = Self::next_tao_id();

			let tao = Tao {
				creator: who.clone(),
				properties: properties.clone(),
			};

			Taos::<T>::insert(tao_id, tao);
			NextTaoId::<T>::mutate(|id| *id += One::one());

			Self::deposit_event(RawEvent::TaoCreated(tao_id, who));

			Ok(())
		}

		#[weight = 0]
		pub fn create_tao_item(origin, tao_id: T::TaoId, is_nf: bool, token_uri: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let _tao = Self::taos(tao_id).ok_or(Error::<T>::InvalidTaoId)?;

			let item_id = Self::next_tao_item_id(tao_id);

			let token_id = token::Module::<T>::create_token(&who, is_nf, &token_uri)?;

			let new_item = TaoItemInfo {
				owner: who.clone(),
				token: token_id,
			};

			TaoItems::<T>::insert(tao_id, item_id, new_item);
			NextTaoItemId::<T>::mutate(tao_id, |id| *id += <TaoItemId as One>::one());

			Self::deposit_event(RawEvent::TaoItemCreated(tao_id, item_id, who));

			Ok(())
		}

	}
}

impl<T: Trait> Module<T> {

}
