#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure,
	StorageValue, StorageMap, Parameter,
};
use frame_system::ensure_signed;
use sp_runtime::{
	traits::{CheckedAdd, CheckedSub, Member, One, Printable, Zero},
	DispatchError, DispatchResult, RuntimeDebug,
};
use sp_std::vec::Vec;

pub type CollectionId = u64;
pub type AssetId = u64;
pub type NftIndex = u64;

/// Collection info
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct CollectionInfo<AccountId, TokenId> {
	/// Class owner
	pub owner: AccountId,
	/// Token id
	pub token: TokenId,
	/// Total issuance for the class
	pub total_supply: u128,
	/// Class Properties
	pub properties: Vec<u8>,
}

/// Token info
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct AssetInfo<AccountId, Data> {
	/// Asset owner
	pub owner: AccountId,
	/// Asset Properties
	pub data: Data,
}

/// Nft Asset info
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct NftAssetData {
	pub name: Vec<u8>,
	pub description: Vec<u8>,
	pub properties: Vec<u8>,
}

/// The pallet's configuration trait.
pub trait Trait: frame_system::Trait + token::Trait {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

pub type CollectionInfoOf<T> =
	CollectionInfo<<T as frame_system::Trait>::AccountId, <T as token::Trait>::TokenId>;
pub type AssetInfoOf<T> = AssetInfo<<T as frame_system::Trait>::AccountId, NftAssetData>;


// This pallet's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as TemplateModule {
		/// Next collection id
		pub NextCollectionId get(fn next_collection_id): CollectionId;
		// Collections
		pub Collections get(fn collections): map hasher(twox_64_concat) CollectionId => Option<CollectionInfoOf<T>>;

		/// Next available asset id per collection.
		pub NextAssetId get(fn next_asset_id): map hasher(twox_64_concat) CollectionId => AssetId;

		pub NftAssets get(fn nft_assets): double_map hasher(twox_64_concat) AssetId, hasher(twox_64_concat) NftIndex => Option<AssetInfoOf<T>>;

		pub NftOwner get(fn nft_owner): double_map hasher(twox_64_concat) AssetId, hasher(twox_64_concat) NftIndex => T::AccountId;
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T> where AccountId = <T as frame_system::Trait>::AccountId {
		CollectionCreated(CollectionId, AccountId),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		NoneValue,
		InvalidCollectionId,
		CollectionNotFound,
		NumOverflow,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 0]
		pub fn create_collection(
			origin,
			token_uri: Vec<u8>,
			properties: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let collection_id = Self::next_collection_id();
			NextCollectionId::mutate(|id| *id += <CollectionId as One>::one());
	
			let token_id = token::Module::<T>::create_token(&who, false, &token_uri)?;
	
			let collection_info = CollectionInfo {
				owner: who.clone(),
				token: token_id,
				total_supply: Default::default(),
				properties,
			};
	
			Collections::<T>::insert(collection_id, collection_info);

			Self::deposit_event(RawEvent::CollectionCreated(collection_id, who));
			Ok(())
		}

		#[weight = 0]
		pub fn mint(
			origin,
			collection_id: CollectionId,
			name: Vec<u8>,
			description: Vec<u8>,
			properties: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			
			let collection = Self::collections(collection_id).ok_or(Error::<T>::InvalidCollectionId)?;
	
			let asset_id = Self::next_asset_id(collection_id);
	
			let new_nft_data = NftAssetData {
				name: name,
				description: description,
				properties: properties,
			};
			let new_nft_data = Into::<NftAssetData>::into(new_nft_data);
	
			let new_asset_info = AssetInfo {
				owner: who.clone(),
				data: new_nft_data,
			};
	
			token::Module::<T>::mint(&who, &collection.token, One::one())?;
	
			Collections::<T>::try_mutate(collection_id, |collection_info| -> DispatchResult {
				let info = collection_info
					.as_mut()
					.ok_or(Error::<T>::CollectionNotFound)?;
				info.total_supply = info
					.total_supply
					.checked_add(One::one())
					.ok_or(Error::<T>::NumOverflow)?;
				Ok(())
			})?;
	
			NftAssets::<T>::insert(collection_id, asset_id, new_asset_info);
			NftOwner::<T>::insert(collection_id, asset_id, who);

			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {

}

// pub trait FungibleAsset<AssetId, AccountId> {
// 	fn create_asset() -> AssetId;
// 	fn mint(asset_id: &AssetId, who: &AccountId, value: Self::Balance) -> DispatchResult;
// 	fn burn(asset_id: &AssetId, who: &AccountId, value: Self::Balance) -> DispatchResult;
// 	fn transfer_from(asset_id: &AssetId, from: &AccountId, to: &AccountId, value: Self::Balance) -> DispatchResult;
// }

// pub trait NonFungibleAsset<CollectionId, AssetId, AccountId> {
// 	fn create_collection() -> Result<CollectionId, DispatchError>;
// 	fn mint(collection_id: &CollectionId, who: &AccountId, value: Self::Balance) -> DispatchResult;
// 	fn burn(collection_id: &CollectionId, who: &AccountId, value: Self::Balance) -> DispatchResult;
// 	fn transfer_from(collection_id: &CollectionId, asset_id: &AssetId, from: &AccountId, to: &AccountId) -> DispatchResult;
// }


// impl<T: Trait> Fungible<T::AssetId, T::AccountId> for Module<T> {
// 	type Balance = T::Balance;

// 	fn create_asset(who: &T::AccountId, uri: Vec<u8>, data: T::AssetData) {
// 		let asset_id = Self::next_asset_id();
// 		NextAssetId::<T>::mutate(|id| *id += One::one());

// 		let token_id = token::Module::<T>::create_token(who.clone(), uri, false)?;

// 		let asset_info = AssetInfo {
// 			owner: who.clone(),
// 			token_id,
// 			data,
// 		};

// 		Assets::<T>::insert(asset_id, asset_info);

// 		asset_id
// 	}

// 	fn mint(asset_id: &AssetId, who: &AccountId, value: Self::Balance) -> DispatchResult {

// 	}
// }

// impl<T: Trait> NonFungibleAsset<T::AssetId, T::AccountId> for Module<T> {
// 	type Balance = T::Balance;

// 	fn create(
// 		who: &T::AccountId,
// 		uri: Vec<u8>,
// 		data: T::CollectionData,
// 	) -> Result<CollectionId, DispatchError> {
// 		let collection_id = NextCollectionId::<T>::mutate(|id| *id += One::one());

// 		let token_id = token::Module::<T>::create_token(who.clone(), uri.clone(), true)?;

// 		let collection_info = CollectionInfo {
// 			owner: who.clone(),
// 			total_issuance: Default::default(),
// 			data,
// 			metadata: uri,
// 		};

// 		Collections::<T>::insert(collection_id, collection_info);

// 		Ok(collection_id)
// 	}
// }
