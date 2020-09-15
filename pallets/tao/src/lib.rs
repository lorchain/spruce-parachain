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
use commodity::{CommodityType, VirtualCommodity, RealCommodity};

pub trait Trait: frame_system::Trait + commodity::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type TaoId: Parameter + Member + AtLeast32Bit + Bounded + Default + Copy;
	type VirtualCommodity: VirtualCommodity<Self::CommodityId, Self::AccountId, Self::TokenBalance>;
	type RealCommodity: RealCommodity<Self::CommodityId, Self::AccountId, Self::TokenBalance>;
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct Tao<AccountId> where {
	owner: AccountId,
	uri: Vec<u8>,
}

/// Commodity creation options.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct CommodityOptions {
	pub is_real: bool,
	pub is_nf: bool,
	pub token_uri: Vec<u8>,
	pub commodity_uri: Vec<u8>,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct CommodityItem<AccountId, CommodityId, TaoId> {
	pub id: CommodityId,
	pub tao: TaoId,
	pub owner: AccountId,
	pub uri: Vec<u8>,
}

decl_storage! {
	trait Store for Module<T: Trait> as TaoModule {
		pub Taos get(fn taos): map hasher(blake2_128_concat) T::TaoId => Option<Tao<T::AccountId>>;
		pub NextTaoId get(fn next_tao_id): T::TaoId;

		pub OwnedTaos get(fn owned_taos): map hasher(blake2_128_concat) (T::AccountId, T::Index) => T::TaoId;
		pub OwnedTaoIndex get(fn owned_tao_index): map hasher(blake2_128_concat) T::AccountId => T::Index;

		pub TaoCommodities get(fn tao_commodities):
			double_map hasher(twox_64_concat) T::TaoId, hasher(blake2_128_concat) T::Index => Option<CommodityItem<T::AccountId, T::CommodityId, T::TaoId>>;
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

			let owned_tao_index = Self::owned_tao_index(sender.clone());
			OwnedTaos::<T>::insert((sender.clone(), owned_tao_index), tao_id);
			OwnedTaoIndex::<T>::mutate(sender.clone(), |index| *index += One::one());

			Self::deposit_event(RawEvent::TaoCreated(tao_id, sender));

			Ok(())
		}

		#[weight = 0]
		pub fn create_commodity(origin, tao_id: T::TaoId, token_id: T::TokenId, commodity_type: CommodityType, uri: Vec<u8>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(Taos::<T>::contains_key(tao_id), Error::<T>::InvalidTaoId);

			let commodity_id = commodity::Module::<T>::create_commodity(&sender, token_id, commodity_type);

			let new_commodity = CommodityItem {
				id: commodity_id,
				tao: tao_id,
				owner: sender.clone(),
				uri,
			};

			let commodity_index = Self::tao_commodity_count();

			TaoCommodities::<T>::insert(tao_id, commodity_index, new_commodity);
			TaoCommodityCount::<T>::mutate(|count| *count += One::one());

			Self::deposit_event(RawEvent::CommodityCreated(tao_id, commodity_id, sender));

			Ok(())
		}

		#[weight = 0]
		pub fn mint(origin, commodity_id: T::CommodityId, amount: T::TokenBalance) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			T::RealCommodity::mint(&commodity_id, &sender, amount);

			Ok(())
		}

	}
}

impl<T: Trait> Module<T> {

}
