#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	debug, decl_module, decl_storage, decl_error, decl_event, ensure, StorageValue, StorageMap, Parameter,
	traits::{Randomness, Currency, Get},
	dispatch::DispatchResult,
};
use frame_system::{self as system, ensure_signed};
use sp_runtime::{
	ModuleId, RuntimeDebug,
	traits::{StaticLookup, AccountIdConversion, AtLeast32Bit, Bounded, Member, Hash, One},
};
use sp_std::{prelude::*, cmp, fmt::Debug, result};

use commodity::{CommodityAsset};

pub trait Trait: frame_system::Trait + token::Trait + commodity::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type TaoId: Parameter + Member + AtLeast32Bit + Bounded + Default + Copy;
	type CommodityAsset: CommodityAsset<Self::CommodityId, Self::AccountId, Self::TokenId, Self::TokenBalance>;
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct Tao<AccountId> where {
	owner: AccountId,
	uri: Vec<u8>,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct Commodity<AccountId, CommodityId, TaoId> {
	pub id: CommodityId,
	pub tao: TaoId,
	pub owner: AccountId,
	pub uri: Vec<u8>,
}

decl_storage! {
	trait Store for Module<T: Trait> as TaoModule {
		pub Taos get(fn taos): map hasher(blake2_128_concat) T::TaoId => Option<Tao<T::AccountId>>;
		pub NextTaoId get(fn next_tao_id): T::TaoId;

		// pub OwnedTaos get(fn owned_taos): map hasher(blake2_128_concat) (T::AccountId, T::Index) => T::TaoId;
		// pub OwnedTaoCount get(fn owned_tao_count): T::Index;
		// pub OwnedTaoIndex get(fn owned_tao_index): map hasher(blake2_128_concat) T::TaoId => T::Index;

		pub TaoCommodities get(fn tao_commodities):
			double_map hasher(twox_64_concat) T::TaoId, hasher(blake2_128_concat) T::Index => Option<Commodity<T::AccountId, T::CommodityId, T::TaoId>>;
		pub TaoCommodityCount get(fn tao_commodity_count): T::Index;
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		InvalidTaoId,
		RequireOwner,
		InvalidProduct,
		InvalidTokenId,
		InsufficientAmount,
	}
}

decl_event!(
	pub enum Event<T> where
		AccountId = <T as frame_system::Trait>::AccountId,
		TaoId = <T as Trait>::TaoId,
		CommodityId = <T as commodity::Trait>::CommodityId,
	{
		TaoCreated(TaoId, AccountId),
		CommodityCreated(TaoId, CommodityId, AccountId),

	}
);

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 0]
		pub fn create_tao(origin, uri: Vec<u8>) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let tao_id = NextTaoId::<T>::get();

			let tao = Tao {
				owner: sender.clone(),
				uri: uri.clone(),
			};

			Taos::<T>::insert(tao_id, tao);
			NextTaoId::<T>::mutate(|id| *id += <T::TaoId as One>::one());

			Self::deposit_event(RawEvent::TaoCreated(tao_id, sender));

			Ok(())
		}

		#[weight = 0]
		pub fn create_commodity(origin, tao_id: T::TaoId, is_real: bool, is_nf: bool, commodity_uri: Vec<u8>, token_uri: Vec<u8>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(Taos::<T>::contains_key(tao_id), Error::<T>::InvalidTaoId);

			let commodity_id = T::CommodityAsset::create_commodity(&sender, is_real, is_nf, token_uri);

			let new_commodity = Commodity {
				id: commodity_id,
				tao: tao_id,
				owner: sender.clone(),
				uri: commodity_uri,
			};

			let commodity_index = Self::tao_commodity_count();

			TaoCommodities::<T>::insert(tao_id, commodity_index, new_commodity);
			TaoCommodityCount::<T>::mutate(|count| *count += One::one());

			Self::deposit_event(RawEvent::CommodityCreated(tao_id, commodity_id, sender));

			Ok(())
		}

	}
}

impl<T: Trait> Module<T> {

}
