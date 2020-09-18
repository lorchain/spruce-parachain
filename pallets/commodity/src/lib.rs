#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure, StorageValue, StorageMap, Parameter,
	traits::{Randomness, Currency, Get},
	dispatch::{DispatchResult, DispatchError},
};
use frame_system::{self as system, ensure_signed};
use sp_runtime::{
	ModuleId, RuntimeDebug,
	traits::{StaticLookup, AccountIdConversion, AtLeast32Bit, Bounded, Member, Hash, One},
};
use sp_std::{prelude::*, cmp, fmt::Debug, result};

/// The pallet's configuration trait.
pub trait Trait: frame_system::Trait + pallet_timestamp::Trait + token::Trait + valley::Trait {
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
	prop: CommodityProperty<AccountId, TokenId, TokenBalance>,
	created: Moment,
}


#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum CommodityProperty<AccountId, TokenId, TokenBalance> {
	RealCommodityProperty(RealCommodityProperty<AccountId, TokenId, TokenBalance>),
	VirtualCommodityProperty(VirtualCommodityProperty),
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct RealCommodityProperty<AccountId, TokenId, TokenBalance> {
	pub reserve: u128,
	pub stake_rate: u128,
	pub duration: u64,
	pub collateral_token: TokenId,
	pub stake_balance: TokenBalance,
	pub stake_minted: TokenBalance,
	pub account: AccountId,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct VirtualCommodityProperty {
	pub reserve: u128,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum CommodityType {
	RealCommodity,
	VirtualCommodity,
}

/// Commodity creation options.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct CommodityOptions<AccountId> {
	pub is_nf: bool,
	pub token_uri: Vec<u8>,
	pub commodity_type: CommodityType,
	pub permissions: PermissionsV1<AccountId>,
}


/// Owner of an asset.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum Owner<AccountId> {
	/// No owner.
	None,
	/// Owned by an AccountId
	Address(AccountId),
}

impl<AccountId> Default for Owner<AccountId> {
	fn default() -> Self {
		Owner::None
	}
}

/// Asset permissions
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct PermissionsV1<AccountId> {
	/// Who have permission to update asset permission
	pub update: Owner<AccountId>,
	///
	pub add_stake: Owner<AccountId>,
	/// Who have permission to mint new asset
	pub mint: Owner<AccountId>,
	/// Who have permission to burn asset
	pub burn: Owner<AccountId>,
}

/// Asset permission types
pub enum PermissionType {
	AddStake,
	/// Permission to burn asset permission
	Burn,
	/// Permission to mint new asset
	Mint,
	/// Permission to update asset
	Update,
}

impl<AccountId> Default for PermissionsV1<AccountId> {
	fn default() -> Self {
		PermissionsV1 {
			update: Owner::None,
			add_stake: Owner::None,
			mint: Owner::None,
			burn: Owner::None,
		}
	}
}


// This pallet's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as CommodityModule {
		pub Commodities get(fn commodities):
			map hasher(blake2_128_concat) T::CommodityId
			=> Option<Commodity<T::AccountId, T::CommodityId, T::TokenId, T::TokenBalance, T::Moment>>;
		
		pub NextCommodityId get(fn next_commodity_id): T::CommodityId;

		pub Permissions get(fn get_permission):
			map hasher(twox_64_concat) T::CommodityId => PermissionsV1<T::AccountId>;
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T> where
		AccountId = <T as frame_system::Trait>::AccountId,
		CommodityId = <T as Trait>::CommodityId,
		Balance = <T as token::Trait>::TokenBalance,
		CommodityOptions = CommodityOptions<<T as frame_system::Trait>::AccountId>,
	{
		Created(CommodityId, AccountId, CommodityOptions),
		PermissionUpdated(CommodityId, PermissionsV1<AccountId>),
		PropertyUpdated(CommodityId, u128, u128, u64),
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
		OnlyVirtualCommodity,
		IdAndTypeMismatch,
		NoMintPermission,
		NoIdAvailable,
		NoUpdatePermission,
		NoStakePermission,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 0]
		pub fn create(origin, options: CommodityOptions<T::AccountId>) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let commodity_id = Self::create_commodity(&sender, &options);
			Self::deposit_event(RawEvent::Created(commodity_id, sender, options));
			Ok(())
		}

		#[weight = 0]
		pub fn update_permission(
			origin,
			commodity_id: T::CommodityId,
			new_permission: PermissionsV1<T::AccountId>
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			if Self::check_permission(&commodity_id, &sender, &PermissionType::Update) {
				<Permissions<T>>::insert(commodity_id, &new_permission);

				Self::deposit_event(RawEvent::PermissionUpdated(commodity_id, new_permission));

				Ok(())
			} else {
				Err(Error::<T>::NoUpdatePermission)?
			}
		}

		#[weight = 0]
		fn update_property(
			origin,
			commodity_id: T::CommodityId,
			reserve: u128,
			stake_rate: u128,
			duration: u64,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			Self::do_update_property(&commodity_id, reserve, stake_rate, duration);

			Self::deposit_event(RawEvent::PropertyUpdated(commodity_id, reserve, stake_rate, duration));
			Ok(())
		}

		#[weight = 0]
		pub fn add_stake(origin, commodity_id: T::CommodityId, amount: T::TokenBalance) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			Self::do_add_stake(&commodity_id, &sender, amount)?;

			Self::deposit_event(RawEvent::AddStake(commodity_id, sender, amount));
			Ok(())
		}


		#[weight = 0]
		pub fn remove_stake(origin, commodity_id: T::CommodityId, amount: T::TokenBalance) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			Self::do_remove_stake(&commodity_id, &sender, amount)?;

			Self::deposit_event(RawEvent::RemoveStake(commodity_id, sender, amount));
			Ok(())
		}


		#[weight = 0]
		pub fn mint(origin, commodity_id: T::CommodityId, amount: T::TokenBalance) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			Self::do_mint(&commodity_id, &sender, amount)?;

			Self::deposit_event(RawEvent::Mint(commodity_id, sender, amount));
			Ok(())
		}

		#[weight = 0]
		pub fn burn(origin, commodity_id: T::CommodityId, amount: T::TokenBalance) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			Self::do_burn(&commodity_id, &sender, amount)?;

			Self::deposit_event(RawEvent::Burn(commodity_id, sender, amount));
			Ok(())
		}

		#[weight = 0]
		pub fn transfer(origin, commodity_id: T::CommodityId, to: T::AccountId, amount: T::TokenBalance) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			Self::do_transfer(&commodity_id, &sender, &to, amount)?;

			Self::deposit_event(RawEvent::Transferred(commodity_id, sender, to, amount));
			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		<T as Trait>::ModuleId::get().into_account()
	}

	pub fn pay_account(sender: &T::AccountId) -> T::AccountId {
		let payload = (
			T::Randomness::random_seed(),
			&sender,
			<frame_system::Module<T>>::extrinsic_index(),
		);
		let hash = payload.using_encoded(T::Hashing::hash);

		<T as Trait>::ModuleId::get().into_sub_account(&hash)
	}

	fn require_amount(rate: u128, amount: T::TokenBalance) -> T::TokenBalance {
		amount * rate.into() / 100.into()
	}

	fn convert_to_collateral(price: u128, amount: T::TokenBalance) -> T::TokenBalance {
		amount * price.into()
	}

	fn check_and_get_commodity(commodity_id: &T::CommodityId, commodity_type: &CommodityType) -> Result<Commodity<T::AccountId, T::CommodityId, T::TokenId, T::TokenBalance, T::Moment>, DispatchError> {
		let commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;
		
		let check_result = match (commodity_type, &commodity.prop) {
			(
				CommodityType::RealCommodity,
				CommodityProperty::RealCommodityProperty(_),
			) => true,
			(
				CommodityType::VirtualCommodity,
				CommodityProperty::VirtualCommodityProperty(_),
			) => true,
			_ => false,
		};
		ensure!(check_result, Error::<T>::IdAndTypeMismatch);

		Ok(commodity)
	}

	pub fn exists(commodity_id: &T::CommodityId) -> bool {
		Self::commodities(commodity_id).is_some()
	}

	pub fn create_commodity(who: &T::AccountId, options: &CommodityOptions<T::AccountId>) -> T::CommodityId {
		let token_id = token::Module::<T>::create_token(who, options.is_nf, &options.token_uri);
		
		let commodity_id = Self::next_commodity_id();
		
		let bei_token_id = valley::Module::<T>::bei_token_id();

		let prop = match options.commodity_type {
			CommodityType::RealCommodity => {
				CommodityProperty::RealCommodityProperty(
					RealCommodityProperty {
						reserve: 0,
						stake_rate: 0,
						duration: 0,
						collateral_token: bei_token_id,
						stake_balance: T::TokenBalance::from(0),
						stake_minted: T::TokenBalance::from(0),
						account: Self::pay_account(&who),
					},
				)
			},
			CommodityType::VirtualCommodity => {
				CommodityProperty::VirtualCommodityProperty(
					VirtualCommodityProperty {
						reserve: 0,
					},
				)
			},
		};

		let new_commodity = Commodity {
			id: commodity_id,
			token: token_id,
			creator: who.clone(),
			prop,
			created: pallet_timestamp::Module::<T>::now(),
		};

		let permissions = options.permissions.clone();

		Commodities::<T>::insert(commodity_id, new_commodity);
		NextCommodityId::<T>::mutate(|id| *id += <T::CommodityId as One>::one());
		Permissions::<T>::insert(commodity_id, permissions);

		commodity_id
	}

	pub fn do_update_property(
		commodity_id: &T::CommodityId,
		reserve: u128,
		stake_rate: u128,
		duration: u64,
	) -> DispatchResult {
		let mut commodity = Self::check_and_get_commodity(commodity_id, &CommodityType::RealCommodity)?;

		match commodity.prop {
			CommodityProperty::RealCommodityProperty(ref mut p) => {
				p.reserve = reserve;
				p.stake_rate = stake_rate;
				p.duration = duration;

				Commodities::<T>::insert(commodity_id, commodity);
			},
			_ => {},
		}

		Ok(())
	}

	pub fn do_add_stake(commodity_id: &T::CommodityId, who: &T::AccountId, amount: T::TokenBalance) -> DispatchResult {
		if Self::check_permission(commodity_id, who, &PermissionType::AddStake) {
			let mut commodity = Self::commodities(commodity_id).ok_or(Error::<T>::InvalidCommodityId)?;

			match commodity.prop {
				CommodityProperty::RealCommodityProperty(ref mut p) => {
					p.stake_balance += amount;

					token::Module::<T>::do_safe_transfer_from(&p.collateral_token, &who, &p.account, amount)?;

					Commodities::<T>::insert(commodity_id, commodity);
				},
				_ => {},
			}

			Ok(())
		} else {
			Err(Error::<T>::NoStakePermission)?
		}
	}

	pub fn do_remove_stake(commodity_id: &T::CommodityId, who: &T::AccountId, amount: T::TokenBalance) -> DispatchResult {
		let mut commodity = Self::check_and_get_commodity(commodity_id, &CommodityType::RealCommodity)?;
		
		match commodity.prop {
			CommodityProperty::RealCommodityProperty(ref mut p) => {
				ensure!(amount < p.stake_balance, Error::<T>::InsufficientAmount);
				p.stake_balance -= amount;

				token::Module::<T>::do_safe_transfer_from(&p.collateral_token, &p.account, &who, amount)?;

				Commodities::<T>::insert(commodity_id, commodity);
			},
			_ => {},
		}

		Ok(())
	}

	pub fn do_mint(
		commodity_id: &T::CommodityId,
		who: &T::AccountId,
		amount: T::TokenBalance,
	) -> DispatchResult {
		if Self::check_permission(commodity_id, who, &PermissionType::Mint) {
			let mut commodity = Self::check_and_get_commodity(commodity_id, &CommodityType::RealCommodity)?;

			match commodity.prop {
				CommodityProperty::RealCommodityProperty(ref mut p) => {
					let expected_available = Self::require_amount(p.stake_rate, amount);
					ensure!(p.stake_balance > expected_available, Error::<T>::InsufficientAmount);
	
					p.stake_balance -= expected_available;
					p.stake_minted += amount;
	
					token::Module::<T>::do_mint(&commodity.token, &who, amount)?;
	
					Commodities::<T>::insert(commodity_id, commodity);
				},
				_ => {
					token::Module::<T>::do_mint(&commodity.token, &who, amount)?;
				},
			}

			Ok(())
		} else {
			Err(Error::<T>::NoMintPermission)?
		}
	}

	pub fn do_burn(commodity_id: &T::CommodityId, who: &T::AccountId, amount: T::TokenBalance) -> DispatchResult {
		let mut commodity = Self::check_and_get_commodity(commodity_id, &CommodityType::RealCommodity)?;

		match commodity.prop {
			CommodityProperty::RealCommodityProperty(ref mut p) => {
				let expected_available = Self::require_amount(p.stake_rate, amount);
				ensure!(p.stake_minted >= amount, Error::<T>::InsufficientBurnAmount);
		
				p.stake_balance += expected_available;
				p.stake_minted -= amount;

				token::Module::<T>::do_burn(&commodity.token, &who, amount)?;

				Commodities::<T>::insert(commodity_id, commodity);
			},
			_ => {
				token::Module::<T>::do_burn(&commodity.token, &who, amount)?;
			},
		}

		Ok(())
	}

	fn redeem(commodity_id: &T::CommodityId, who: &T::AccountId, amount: T::TokenBalance) -> DispatchResult {
		let mut commodity = Self::check_and_get_commodity(commodity_id, &CommodityType::RealCommodity)?;

		match commodity.prop {
			CommodityProperty::RealCommodityProperty(ref mut p) => {
				ensure!(p.stake_minted >= amount, Error::<T>::InsufficientAmount);
				let expected_available = Self::require_amount(p.stake_rate, amount);

				let token_to_collateral = Self::convert_to_collateral(p.reserve, amount);
				p.stake_balance = p.stake_balance + expected_available - token_to_collateral;
				p.stake_minted -= amount;

				token::Module::<T>::do_burn(&commodity.token, &who, amount)?;
				token::Module::<T>::do_safe_transfer_from(&p.collateral_token, &p.account, &who, token_to_collateral)?;

				Commodities::<T>::insert(commodity_id, commodity);
			},
			_ => {},
		}

		Ok(())
	}

	pub fn do_transfer(commodity_id: &T::CommodityId, from: &T::AccountId, to: &T::AccountId, amount: T::TokenBalance) -> DispatchResult {
		let commodity = Self::check_and_get_commodity(commodity_id, &CommodityType::VirtualCommodity)?;

		token::Module::<T>::do_safe_transfer_from(&commodity.token, &from, &to, amount)?;

		Ok(())
	}

	pub fn check_permission(commodity_id: &T::CommodityId, who: &T::AccountId, what: &PermissionType) -> bool {
		let permission = Self::get_permission(commodity_id);

		match (what, permission) {
			(
				PermissionType::AddStake,
				PermissionsV1 {
					add_stake: Owner::Address(account),
					..
				},
			) => account == *who,
			(
				PermissionType::Burn,
				PermissionsV1 {
					burn: Owner::Address(account),
					..
				},
			) => account == *who,
			(
				PermissionType::Mint,
				PermissionsV1 {
					mint: Owner::Address(account),
					..
				},
			) => account == *who,
			(
				PermissionType::Update,
				PermissionsV1 {
					update: Owner::Address(account),
					..
				},
			) => account == *who,
			_ => false,
		}
	}

}
