#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure, StorageValue, StorageMap, Parameter,
	traits::{Randomness, Currency, Get},
	dispatch,
};
use frame_system::{self as system, ensure_signed};
use sp_runtime::{
	ModuleId,
	traits::{StaticLookup, AccountIdConversion, AtLeast32Bit, Bounded, Member, Hash, One},
};

/// The pallet's configuration trait.
pub trait Trait: frame_system::Trait + pallet_timestamp::Trait + tao::Trait + valley::Trait {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	type CommodityId: Parameter + Member + AtLeast32Bit + Bounded + Default + Copy;
}

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub struct Commodity<T> where
	T: Trait
{
	tao: T::TaoId,
	token: T::TokenId,
	price: u128,
	count: u64,
	stake_rate: u128,
	stake_balance: T::TokenBalance,
	stake_minted: T::TokenBalance,
	account: T::AccountId,
	created: T::Moment,
}

// This pallet's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as CommodityModule {
		pub Commodities get(fn commodities): map hasher(blake2_128_concat) T::CommodityId => Option<Commodity<T>>;
		pub NextCommodityId get(fn next_commodity_id): T::CommodityId;
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T> where
		AccountId = <T as frame_system::Trait>::AccountId,
		CommodityId = <T as Trait>::CommodityId,
	{
		CommodityCreated(AccountId, CommodityId),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		NoneValue,
		StorageOverflow,
		InsufficientAmount,
		InvalidCommodityId,
		InsufficientBurnAmount,
		InsufficientRedeemAmount,
		InvalidTokenId,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 10_000 + T::DbWeight::get().writes(1)]
		pub fn create_commodity(origin, token_id: T::TokenId, price: u128, count: u64) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			let token = tao::Tokens::<T>::get(token_id).unwrap();
			ensure!(token.is_commodity == true, Error::<T>::InvalidTokenId);

			let commodity_id = Self::next_commodity_id();

			let account = Self::pay_account(&sender);

			let new_commodity = Commodity::<T>{
				tao: token.tao,
				token: token_id,
				price,
				count,
				stake_rate: 0,
				stake_balance: T::TokenBalance::from(0),
				stake_minted: T::TokenBalance::from(0),
				account,
				created: pallet_timestamp::Module::<T>::now(),
			};

			Commodities::<T>::insert(commodity_id, new_commodity);

			Self::deposit_event(RawEvent::CommodityCreated(sender, commodity_id));
			Ok(())
		}

		#[weight = 0]
		pub fn add_stake_to_commodity(origin, commodity_id: T::CommodityId, amount: T::TokenBalance) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			let mut commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;
			let bei_token_id = valley::Module::<T>::bei_token_id();

			token::Module::<T>::do_safe_transfer_from(sender.clone(), commodity.account.clone(), bei_token_id, amount)?;

			commodity.stake_balance += amount;
			Commodities::<T>::insert(commodity_id, commodity);

			Ok(())
		}

		#[weight = 0]
		pub fn remove_stake_from_commodity(origin, commodity_id: T::CommodityId, amount: T::TokenBalance) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			let mut commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;
			ensure!(amount < commodity.stake_balance, Error::<T>::InsufficientAmount);

			let bei_token_id = valley::Module::<T>::bei_token_id();

			token::Module::<T>::do_safe_transfer_from(commodity.account.clone(), sender.clone(), bei_token_id, amount)?;

			commodity.stake_balance -= amount;
			Commodities::<T>::insert(commodity_id, commodity);

			Ok(())
		}

		#[weight = 0]
		pub fn mint(origin, commodity_id: T::CommodityId, to: T::AccountId, amount: T::TokenBalance) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			let mut commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;
			let expected_available = Self::require_amount(commodity.stake_rate, amount);
			ensure!(commodity.stake_balance > expected_available, Error::<T>::InsufficientAmount);

			commodity.stake_balance -= expected_available;
			commodity.stake_minted += amount;

			tao::Module::<T>::do_mint(commodity.tao, commodity.token, to, amount)?;

			Ok(())
		}

		#[weight = 0]
		pub fn burn(origin, commodity_id: T::CommodityId, commodity_token: T::TokenId, amount: T::TokenBalance) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			// TODO: check commodity_token for this commodity and burn time

			let mut commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;
			let expected_available = Self::require_amount(commodity.stake_rate, amount);
			ensure!(commodity.stake_minted >= amount, Error::<T>::InsufficientBurnAmount);

			commodity.stake_balance += expected_available;
			commodity.stake_minted -= amount;

			tao::Module::<T>::do_burn(commodity.tao, commodity_token, sender, amount)?;

			Ok(())
		}

		#[weight = 0]
		pub fn redeem(origin, commodity_id: T::CommodityId, commodity_token: T::TokenId, amount: T::TokenBalance) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			// TODO: check commodity redeem time

			let mut commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;
			ensure!(commodity.stake_minted >= amount, Error::<T>::InsufficientRedeemAmount);
			let expected_available = Self::require_amount(commodity.stake_rate, amount);

			let token_to_bei = Self::convert_token_to_bei(commodity.price, amount);

			commodity.stake_balance = commodity.stake_balance + expected_available - token_to_bei;
			commodity.stake_minted -= amount;

			let bei_token_id = valley::Module::<T>::bei_token_id();

			tao::Module::<T>::do_burn(commodity.tao, commodity_token, sender.clone(), amount)?;
			token::Module::<T>::do_safe_transfer_from(commodity.account.clone(), sender.clone(), bei_token_id, token_to_bei)?;

			Commodities::<T>::insert(commodity_id, commodity);

			Ok(())
		}

	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
	}

	pub fn pay_account(sender: &T::AccountId) -> T::AccountId {
		let payload = (
			T::Randomness::random_seed(),
			&sender,
			<frame_system::Module<T>>::extrinsic_index(),
		);
		let hash = payload.using_encoded(T::Hashing::hash);

		T::ModuleId::get().into_sub_account(&hash)
	}

	pub fn require_amount(rate: u128, amount: T::TokenBalance) -> T::TokenBalance {
		amount * rate.into() / 100.into()
	}

	pub fn convert_token_to_bei(price: u128, amount: T::TokenBalance) -> T::TokenBalance {
		amount * price.into()
	}


}
