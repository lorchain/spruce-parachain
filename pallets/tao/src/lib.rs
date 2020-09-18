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
	traits::{
		StaticLookup, AccountIdConversion, AtLeast32Bit, Bounded, Member, Hash, One, CheckedAdd, CheckedSub,
	},
};
use sp_std::{prelude::*, cmp, fmt::Debug, result};
use commodity::CommodityOptions;

pub trait Trait: frame_system::Trait + commodity::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type TaoId: Parameter + Member + AtLeast32Bit + Bounded + Default + Copy;
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct Tao<AccountId> where {
	owner: AccountId,
	uri: Vec<u8>,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct TaoCommodity<AccountId, CommodityId, TaoId> {
	pub id: CommodityId,
	pub tao: TaoId,
	pub owner: AccountId,
}

decl_storage! {
	trait Store for Module<T: Trait> as TaoModule {
		pub Taos get(fn taos): map hasher(blake2_128_concat) T::TaoId => Option<Tao<T::AccountId>>;
		pub NextTaoId get(fn next_tao_id): T::TaoId;

		pub TaoCommodities get(fn tao_commodities):
			double_map hasher(twox_64_concat) T::TaoId, hasher(blake2_128_concat) T::Index => Option<TaoCommodity<T::AccountId, T::CommodityId, T::TaoId>>;
		pub TaoCommodityIndex get(fn tao_commodity_index):
			double_map hasher(twox_64_concat) T::TaoId, hasher(blake2_128_concat) T::CommodityId => T::Index;
		pub TaoCommodityCount get(fn tao_commodity_count): map hasher(blake2_128_concat) T::TaoId => T::Index;
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		InvalidTaoId,
		InvalidCommodityId,
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
		TaoCreated(TaoId, AccountId, Vec<u8>),
		CommodityAdded(TaoId, CommodityId, AccountId),
		CommodityRemoved(TaoId, CommodityId, AccountId),
	}
);

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 0]
		pub fn create(origin, uri: Vec<u8>) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let tao_id = Self::next_tao_id();

			let tao = Tao {
				owner: sender.clone(),
				uri: uri.clone(),
			};

			Taos::<T>::insert(tao_id, tao);
			NextTaoId::<T>::mutate(|id| *id += One::one());

			Self::deposit_event(RawEvent::TaoCreated(tao_id, sender, uri));

			Ok(())
		}

		#[weight = 0]
		pub fn create_tao_commodity(origin, tao_id: T::TaoId, commodity_options: CommodityOptions<T::AccountId>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(Taos::<T>::contains_key(tao_id), Error::<T>::InvalidTaoId);

			let commodity_id = commodity::Module::<T>::create_commodity(&sender, &commodity_options);

			let new_commodity = TaoCommodity {
				id: commodity_id,
				tao: tao_id,
				owner: sender.clone(),
			};

			let new_commodity_index = Self::tao_commodity_count(tao_id);

			TaoCommodities::<T>::insert(tao_id, new_commodity_index, new_commodity);
			TaoCommodityCount::<T>::mutate(tao_id, |count| *count += One::one());
			TaoCommodityIndex::<T>::insert(tao_id, commodity_id, new_commodity_index);

			Self::deposit_event(RawEvent::CommodityAdded(tao_id, commodity_id, sender));

			Ok(())
		}

		#[weight = 0]
		pub fn remove_commodity(origin, tao_id: T::TaoId, commodity_id: T::CommodityId) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(Taos::<T>::contains_key(tao_id), Error::<T>::InvalidTaoId);

			let commodity_index = Self::tao_commodity_index(tao_id, commodity_id);

			TaoCommodities::<T>::remove(tao_id, commodity_index);
			TaoCommodityCount::<T>::mutate(tao_id, |count| *count -= One::one());
			TaoCommodityIndex::<T>::remove(tao_id, commodity_id);

			Self::deposit_event(RawEvent::CommodityRemoved(tao_id, commodity_id, sender));

			Ok(())
		}

	}
}

impl<T: Trait> Module<T> {

}
