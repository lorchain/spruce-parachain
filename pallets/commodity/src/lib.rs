#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure, StorageValue, StorageMap, Parameter,
	traits::{Randomness, Currency, Get},
	dispatch::DispatchResult,
};
use frame_system::{self as system, ensure_signed};
use sp_runtime::{
	ModuleId, RuntimeDebug,
	traits::{StaticLookup, AccountIdConversion, AtLeast32Bit, Bounded, Member, Hash, One},
};
use sp_std::{prelude::*, cmp, fmt::Debug, result};

/// The pallet's configuration trait.
pub trait Trait: frame_system::Trait + pallet_timestamp::Trait + token::Trait {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	type ModuleId: Get<ModuleId>;
	type CommodityId: Parameter + Member + AtLeast32Bit + Bounded + Default + Copy;
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct Commodity<AccountId, CommodityId, TokenId, TokenBalance, Moment>
{
	id: CommodityId,
	creator: AccountId,
	token: TokenId,
	mode: CommodityMode<AccountId, TokenId, TokenBalance>,
	created: Moment,
}


#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum CommodityMode<AccountId, TokenId, TokenBalance> {
	RealCommodity(RealCommodity<AccountId, TokenId, TokenBalance>),
	VirtualCommodity(VirtualCommodity),
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct RealCommodity<AccountId, TokenId, TokenBalance> {
	pub reserve: u128,
	pub stake_rate: u128,
	pub duration: u64,
	pub collateral_token: TokenId,
	pub stake_balance: TokenBalance,
	pub stake_minted: TokenBalance,
	pub account: AccountId,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct VirtualCommodity {
	pub reserve: u128,
}


// This pallet's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as CommodityModule {
		pub Commodities get(fn commodities):
			map hasher(blake2_128_concat) T::CommodityId
			=> Option<Commodity<T::AccountId, T::CommodityId, T::TokenId, T::TokenBalance, T::Moment>>;
		
		pub NextCommodityId get(fn next_commodity_id): T::CommodityId;
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T> where
		AccountId = <T as frame_system::Trait>::AccountId,
		CommodityId = <T as Trait>::CommodityId,
		Balance = <T as token::Trait>::TokenBalance,
	{
		AddStake(CommodityId, AccountId, Balance),
		RemoveStake(CommodityId, AccountId, Balance),
		Mint(CommodityId, AccountId, Balance),
		Burn(CommodityId, AccountId, Balance),
		Redeem(CommodityId, AccountId, Balance),
		Transferred(CommodityId, AccountId, AccountId, Balance),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		NoneValue,
		InsufficientAmount,
		InsufficientBurnAmount,
		InvalidCommodityId,
		OnlyRealCommodity,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 0]
		fn set_real_commodity(
			origin,
			id: T::CommodityId,
			reserve: u128,
			stake_rate: u128,
			duration: u64,
			collateral_token: T::TokenId,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			<Self as CommodityAsset<_, _, _, _>>::set_real_commodity(&id, &sender, reserve, stake_rate, duration, collateral_token)?;

			Ok(())
		}

		#[weight = 0]
		pub fn add_stake(origin, commodity_id: T::CommodityId, amount: T::TokenBalance) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			<Self as CommodityAsset<_, _, _, _>>::add_stake(&commodity_id, &sender, amount)?;

			Self::deposit_event(RawEvent::AddStake(commodity_id, sender, amount));

			Ok(())
		}


		#[weight = 0]
		pub fn remove_stake(origin, commodity_id: T::CommodityId, amount: T::TokenBalance) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			<Self as CommodityAsset<_, _, _, _>>::remove_stake(&commodity_id, &sender, amount)?;

			Self::deposit_event(RawEvent::RemoveStake(commodity_id, sender, amount));

			Ok(())
		}


		#[weight = 0]
		pub fn mint(origin, commodity_id: T::CommodityId, amount: T::TokenBalance) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			<Self as CommodityAsset<_, _, _, _>>::mint(&commodity_id, &sender, amount)?;

			Self::deposit_event(RawEvent::Mint(commodity_id, sender, amount));

			Ok(())
		}

		#[weight = 0]
		pub fn burn(origin, commodity_id: T::CommodityId, amount: T::TokenBalance) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			<Self as CommodityAsset<_, _, _, _>>::burn(&commodity_id, &sender, amount)?;

			Self::deposit_event(RawEvent::Burn(commodity_id, sender, amount));

			Ok(())
		}

		#[weight = 0]
		pub fn transfer(origin, commodity_id: T::CommodityId, to: T::AccountId, amount: T::TokenBalance) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			<Self as CommodityAsset<_, _, _, _>>::transfer(&commodity_id, &sender, &to, amount)?;

			Self::deposit_event(RawEvent::Transferred(commodity_id, sender, to, amount));

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

	fn require_amount(rate: u128, amount: T::TokenBalance) -> T::TokenBalance {
		amount * rate.into() / 100.into()
	}

	fn convert_to_collateral(price: u128, amount: T::TokenBalance) -> T::TokenBalance {
		amount * price.into()
	}
}


pub trait CommodityAsset<CommodityId, AccountId, TokenId, TokenBalance> {
	fn exists(commodity_id: &CommodityId) -> bool;

	fn create_commodity(who: &AccountId, is_virtual: bool, is_nf: bool, token_uri: Vec<u8>) -> CommodityId;

	fn set_real_commodity(commodity_id: &CommodityId, who: &AccountId, reserve: u128, stake_rate: u128, duration: u64, collateral_token: TokenId) -> DispatchResult;

	fn add_stake(commodity_id: &CommodityId, who: &AccountId, amount: TokenBalance) -> DispatchResult;

	fn remove_stake(commodity_id: &CommodityId, who: &AccountId, amount: TokenBalance) -> DispatchResult;

	fn mint(commodity_id: &CommodityId, who: &AccountId, amount: TokenBalance) -> DispatchResult;

	fn burn(commodity_id: &CommodityId, who: &AccountId, amount: TokenBalance) -> DispatchResult;

	fn redeem(commodity_id: &CommodityId, who: &AccountId, amount: TokenBalance) -> DispatchResult;

	fn transfer(commodity_id: &CommodityId, from: &AccountId, to: &AccountId, amount: TokenBalance) -> DispatchResult;
}


impl<T: Trait> CommodityAsset<T::CommodityId, T::AccountId, T::TokenId, T::TokenBalance> for Module<T> {

	fn exists(commodity_id: &T::CommodityId) -> bool {
		Self::commodities(commodity_id).is_some()
	}

	fn create_commodity(who: &T::AccountId, is_real: bool, is_nf: bool, token_uri: Vec<u8>) -> T::CommodityId {
		let token_id = token::Module::<T>::create_token(&who, is_nf, token_uri);

		let mode = if is_real == true {
			CommodityMode::RealCommodity(
				RealCommodity {
					reserve: 0,
					stake_rate: 0,
					duration: 0,
					collateral_token: T::TokenId::from([0; 32]),
					stake_balance: T::TokenBalance::from(0),
					stake_minted: T::TokenBalance::from(0),
					account: Self::pay_account(&who),
				},
			)
		} else {
			CommodityMode::VirtualCommodity(
				VirtualCommodity {
					reserve: 0,
				},
			)
		};
		
		let commodity_id = Self::next_commodity_id();

		let new_commodity = Commodity {
			id: commodity_id,
			token: token_id,
			creator: who.clone(),
			mode,
			created: pallet_timestamp::Module::<T>::now(),
		};

		Commodities::<T>::insert(commodity_id, new_commodity);
		NextCommodityId::<T>::mutate(|id| *id += <T::CommodityId as One>::one());

		commodity_id
	}

	fn set_real_commodity(
		commodity_id: &T::CommodityId,
		who: &T::AccountId,
		reserve: u128,
		stake_rate: u128,
		duration: u64,
		collateral_token: T::TokenId,
	) -> DispatchResult {
		let mut commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;

		match commodity.mode {
			CommodityMode::RealCommodity(ref mut p) => {
				p.reserve = reserve;
				p.stake_rate = stake_rate;
				p.duration = duration;
				p.collateral_token = collateral_token;

				Commodities::<T>::insert(commodity_id, commodity);
			},
			_ => {},
		}

		Ok(())
	}

	fn add_stake(commodity_id: &T::CommodityId, who: &T::AccountId, amount: T::TokenBalance) -> DispatchResult {
		let mut commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;

		match commodity.mode {
			CommodityMode::RealCommodity(ref mut p) => {
				p.stake_balance += amount;

				token::Module::<T>::do_safe_transfer_from(&p.collateral_token, &who, &p.account, amount)?;

				Commodities::<T>::insert(commodity_id, commodity);
			},
			_ => {},
		}

		Ok(())
	}

	fn remove_stake(commodity_id: &T::CommodityId, who: &T::AccountId, amount: T::TokenBalance) -> DispatchResult {
		let mut commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;
		
		match commodity.mode {
			CommodityMode::RealCommodity(ref mut p) => {
				ensure!(amount < p.stake_balance, Error::<T>::InsufficientAmount);
				p.stake_balance -= amount;

				token::Module::<T>::do_safe_transfer_from(&p.collateral_token, &p.account, &who, amount)?;

				Commodities::<T>::insert(commodity_id, commodity);
			},
			_ => {},
		}

		Ok(())
	}

	fn mint(commodity_id: &T::CommodityId, who: &T::AccountId, amount: T::TokenBalance) -> DispatchResult {
		let mut commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;

		match commodity.mode {
			CommodityMode::RealCommodity(ref mut p) => {
				let expected_available = Self::require_amount(p.stake_rate, amount);
				ensure!(p.stake_balance > expected_available, Error::<T>::InsufficientAmount);
				p.stake_balance -= expected_available;
				p.stake_minted += amount;

				token::Module::<T>::mint(&commodity.token, &who, amount)?;

				Commodities::<T>::insert(commodity_id, commodity);
			},
			CommodityMode::VirtualCommodity(_) => {
				token::Module::<T>::mint(&commodity.token, &who, amount)?;
			},
		}

		Ok(())
	}

	fn burn(commodity_id: &T::CommodityId, who: &T::AccountId, amount: T::TokenBalance) -> DispatchResult {
		let mut commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;

		match commodity.mode {
			CommodityMode::RealCommodity(ref mut p) => {
				let expected_available = Self::require_amount(p.stake_rate, amount);
				ensure!(p.stake_minted >= amount, Error::<T>::InsufficientBurnAmount);
		
				p.stake_balance += expected_available;
				p.stake_minted -= amount;

				token::Module::<T>::burn(&commodity.token, &who, amount)?;

				Commodities::<T>::insert(commodity_id, commodity);
			},
			CommodityMode::VirtualCommodity(_) => {
				token::Module::<T>::burn(&commodity.token, &who, amount)?;
			},
		}

		Ok(())
	}

	fn redeem(commodity_id: &T::CommodityId, who: &T::AccountId, amount: T::TokenBalance) -> DispatchResult {
		let mut commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;

		match commodity.mode {
			CommodityMode::RealCommodity(ref mut p) => {
				ensure!(p.stake_minted >= amount, Error::<T>::InsufficientAmount);
				let expected_available = Self::require_amount(p.stake_rate, amount);

				let token_to_collateral = Self::convert_to_collateral(p.reserve, amount);
				p.stake_balance = p.stake_balance + expected_available - token_to_collateral;
				p.stake_minted -= amount;

				token::Module::<T>::burn(&commodity.token, &who, amount)?;
				token::Module::<T>::do_safe_transfer_from(&p.collateral_token, &p.account, &who, token_to_collateral)?;

				Commodities::<T>::insert(commodity_id, commodity);
			},
			_ => {},
		}

		Ok(())
	}

	fn transfer(commodity_id: &T::CommodityId, from: &T::AccountId, to: &T::AccountId, amount: T::TokenBalance) -> DispatchResult {
		let commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;

		match commodity.mode {
			CommodityMode::RealCommodity(_) => {},
			CommodityMode::VirtualCommodity(_) => {
				token::Module::<T>::do_safe_transfer_from(&commodity.token, &from, &to, amount)?;
			},
		}

		Ok(())
	}

}
