#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use frame_support::{
	debug, decl_module, decl_storage, decl_error, decl_event, ensure, StorageValue, StorageMap, Parameter,
	traits::Randomness,
	dispatch,
};
use sp_io::hashing::blake2_128;
use frame_system::{self as system, ensure_signed};
use sp_runtime::{traits::{
	AtLeast32Bit, MaybeSerializeDeserialize, MaybeDisplay, Bounded, Member,
	SimpleBitOps, CheckEqual, MaybeSerialize, MaybeMallocSizeOf, Hash, One,
	Zero, AtLeast32BitUnsigned,
}};
use sp_std::fmt::Debug;
use sp_std::prelude::*;
use sp_std::vec::Vec;

/// The module's configuration trait.
pub trait Trait: frame_system::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	/// Currency of this module
	// type Currency: Currency<Self::AccountId>;
	type Randomness: Randomness<Self::Hash>;
	// type TokenId: Parameter + Member + Default + Copy;

	// type TokenBalance: Parameter + Member + AtLeast32BitUnsigned + Default + Copy
	//     + MaybeSerializeDeserialize;
	
	type TokenBalance: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + Debug +
		MaybeSerializeDeserialize + From<u128> + Into<u128>;
    
    type TokenId: Parameter + Member + Debug + Default + Copy + Ord
		+ MaybeSerializeDeserialize + From<[u8;32]> + Into<[u8;32]>;

	type TokenType: Parameter + Member + Debug + Default + Copy + Ord
        + MaybeSerializeDeserialize + From<[u8;16]> + Into<[u8;16]>;

	// type TokenIndex: Parameter + Member + AtLeast32Bit + Default + Copy
    //     + MaybeSerializeDeserialize;

	// type TokenId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + SimpleBitOps + Ord
	// + Default + Copy + CheckEqual + sp_std::hash::Hash + AsRef<[u8]> + AsMut<[u8]> + MaybeMallocSizeOf;
}

// pub type TokenId<T> = <T as frame_system::Trait>::Hash;
// pub type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub struct Token<T> where
	T: Trait
{
	token_id: T::TokenId,
	creator: T::AccountId,
	is_nf: bool,
	uri: Vec<u8>,
}

decl_storage! {
    trait Store for Module<T: Trait> as TokenModule {
		pub Tokens get(fn tokens): map hasher(blake2_128_concat) T::TokenType => Option<Token<T>>;
		pub TokenCount get(fn token_count): u64;
		pub NFIndex get(fn nf_index): map hasher(blake2_128_concat) T::TokenType => u128;

		// ERC1155
		pub Balances get(fn balance_of): map hasher(blake2_128_concat) (T::TokenId, T::AccountId) => T::TokenBalance;
		pub OperatorApproval get(fn operator_approval): map hasher(blake2_128_concat) (T::AccountId, T::AccountId) => bool;
		pub TotalSupply get(fn total_supply): map hasher(blake2_128_concat) T::TokenId => T::TokenBalance;
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		InvalidTokenId,
		RequireNonFungible,
		RequireFungible,
		LengthMismatch,
		InsufficientFunds,
	}
}

decl_event!(
    pub enum Event<T> where
		AccountId = <T as frame_system::Trait>::AccountId,
		TokenId = <T as Trait>::TokenId,
		TokenBalance = <T as Trait>::TokenBalance,
    {
		Created(AccountId, TokenId),
		MintNonFungible(TokenId, AccountId, TokenId),
		MintFungible(TokenId, AccountId, TokenBalance),
		Burn(TokenId, AccountId, TokenBalance),
		BurnBatch(TokenId, Vec<AccountId>, Vec<TokenBalance>),
		Transfer(AccountId, AccountId, TokenId, TokenBalance),
        Approval(AccountId, AccountId, TokenBalance),
    }
);

decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 0]
        fn set_approval_for_all(origin, operator: T::AccountId, approved: bool) -> dispatch::DispatchResult {
           let sender = ensure_signed(origin)?;

		   OperatorApproval::<T>::mutate((sender, operator), |approval| *approval = approved);

           Ok(())
        }

		#[weight = 0]
        fn safe_transfer_from(
			origin,
			to: T::AccountId,
			id: T::TokenId,
			amount: T::TokenBalance
		) -> dispatch::DispatchResult 
		{
			let sender = ensure_signed(origin)?;

			Self::do_safe_transfer_from(sender, to, id, amount)?;

			Ok(())
		}

		#[weight = 0]
        fn safe_batch_transfer_from(
			origin,
			to: T::AccountId,
			ids: Vec<T::TokenId>,
			amounts: Vec<T::TokenBalance>
		) -> dispatch::DispatchResult 
		{
			let sender = ensure_signed(origin)?;

			for i in 0..ids.len() {
				let id = ids[i];
				let amount = amounts[i];

				Self::do_safe_transfer_from(sender.clone(), to.clone(), id, amount)?;
			}

			Ok(())
		}

		#[weight = 0]
		fn debug_create_token(origin, is_nf: bool) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			debug::info!("run into crate token");

			let id = Self::create_token(&sender, is_nf, [].to_vec());
			// let id = Self::get_token_id(&sender);
			debug::info!("id is {:?}", id);

			Ok(())
		}

		#[weight = 0]
		fn debug_mint_nf(origin, token_id: T::TokenId, to: Vec<T::AccountId>) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			debug::info!("run into mint nf");

			Self::mint_non_fungible(token_id, &to)?;

			Ok(())
		}

		#[weight = 0]
		fn debug_mint_f(origin, token_id: T::TokenId, to: Vec<T::AccountId>, amounts: Vec<T::TokenBalance>) -> dispatch::DispatchResult {
			let sender = ensure_signed(origin)?;

			debug::info!("run into mint nf");

			Self::mint_fungible(token_id, &to, amounts)?;

			Ok(())
		}

    }
}

impl<T: Trait> Module<T> {
	// fn get_token_type(sender: &T::AccountId) -> T::TokenType {
	// 	let payload = (
	// 		T::Randomness::random_seed(),
	// 		&sender,
	// 		<frame_system::Module<T>>::extrinsic_index(),
	// 	);

	// 	let random = payload.using_encoded(blake2_128);

	// 	let mut array = [0; 32];
	// 	array[..16].copy_from_slice(&random[..]);
	// 	debug::info!("array is {:?}", array);

	// 	array.into()
	// }

	fn random_type(sender: &T::AccountId) -> T::TokenType {
		let payload = (
			T::Randomness::random_seed(),
			&sender,
			<frame_system::Module<T>>::extrinsic_index(),
		);

		payload.using_encoded(blake2_128).into()
	}

	fn get_token_id(token_type: T::TokenType, index: u128) -> T::TokenId {
		let type_bytes = token_type.into();
		let index_bytes = index.to_be_bytes();

		let mut array = [0 as u8; 32];
		array[..16].copy_from_slice(&type_bytes[..16]);
		array[16..].copy_from_slice(&index_bytes[..16]);

		array.into()
	}

	fn convert_id_to_type(token_id: T::TokenId) -> T::TokenType {
		let id_bytes = token_id.into();

		let mut type_array = [0 as u8; 16];
		type_array.copy_from_slice(&id_bytes[..16]);

		type_array.into()
	}

	pub fn is_nf_token(token_id: T::TokenId) -> bool {
		let token_type = Self::convert_id_to_type(token_id);
		let token = Self::tokens(token_type).unwrap();
		token.is_nf
	}

	pub fn create_token(who: &T::AccountId, is_nf: bool, uri: Vec<u8>) -> T::TokenId {
		let token_type = Self::random_type(&who);
		let token_id = Self::get_token_id(token_type, 0 as u128);

		debug::info!("is nf {}", is_nf);

		let token = Token::<T> {
			token_id,
			creator: who.clone(),
			is_nf,
			uri: uri.clone(),
		};

		Tokens::<T>::insert(token_type, token);
		TokenCount::mutate(|id| *id += <u64 as One>::one());

		Self::deposit_event(RawEvent::Created(who.clone(), token_id));

		token_id
	}
	
	pub fn mint_non_fungible(
		token_id: T::TokenId,
		to: &Vec<T::AccountId>,
	) -> Result<Vec<T::TokenId>, dispatch::DispatchError> {
		let token_type = Self::convert_id_to_type(token_id);
		
		let token = Self::tokens(token_type).ok_or(Error::<T>::InvalidTokenId)?;
		ensure!(token.is_nf == true, Error::<T>::RequireNonFungible);

		let index = Self::nf_index(token_type).checked_add(<u128 as One>::one()).unwrap();
		NFIndex::<T>::mutate(token_type, |index| *index += to.len() as u128);

		let nf_id = Self::get_token_id(token_type, 0 as u128);

		debug::info!("index is {}", index);
		debug::info!("to len is {}", to.len());

		let mut id_vec = Vec::new();

		for i in 0..to.len() {
			let account = &to[i];
			let amount = T::TokenBalance::from(1);
			let id = Self::get_token_id(token_type, index + i as u128);
			id_vec.push(id);

			Balances::<T>::mutate((id, account), |balance| *balance += amount.clone());
			TotalSupply::<T>::mutate(id, |supply| *supply += amount.clone());
			
			TotalSupply::<T>::mutate(nf_id, |supply| *supply += amount);

			Self::deposit_event(RawEvent::MintNonFungible(token_id, account.clone(), id));
		}

		Ok(id_vec)
	}

	pub fn mint_fungible(
		token_id: T::TokenId,
		to: &Vec<T::AccountId>,
		amounts: Vec<T::TokenBalance>,
	) -> Result<(), dispatch::DispatchError> {
		ensure!(to.len() == amounts.len(), Error::<T>::LengthMismatch);

		let token_type = Self::convert_id_to_type(token_id);

		let token = Self::tokens(token_type).ok_or(Error::<T>::InvalidTokenId)?;
		ensure!(token.is_nf == false, Error::<T>::RequireFungible);

		debug::info!("to len is {}", to.len());

		for i in 0..to.len() {
			let account = &to[i];
			let amount = amounts[i];

			Balances::<T>::mutate((token_id, account), |balance| *balance += amount.clone());
			TotalSupply::<T>::mutate(token_id, |supply| *supply += amount);

			Self::deposit_event(RawEvent::MintFungible(token_id, account.clone(), amount));
		}

		Ok(())
	}

	pub fn mint(token_id: T::TokenId, to: &T::AccountId, amount: T::TokenBalance) -> Result<(), dispatch::DispatchError>  {
		let is_nf = Self::is_nf_token(token_id);

		if is_nf {
			Self::mint_non_fungible(token_id, &[ to.clone() ].to_vec());
		} else {
			Self::mint_fungible(token_id, &[ to.clone() ].to_vec(), [ amount ].to_vec());
		}

		Ok(())
	}

	pub fn mint_batch(token_id: T::TokenId, to: &Vec<T::AccountId>, amounts: Vec<T::TokenBalance>) -> Result<(), dispatch::DispatchError>  {
		let is_nf = Self::is_nf_token(token_id);

		if is_nf {
			Self::mint_non_fungible(token_id, &to);
		} else {
			Self::mint_fungible(token_id, &to, amounts);
		}

		Ok(())
	}

	pub fn burn(token_id: T::TokenId, to: &T::AccountId, amount: T::TokenBalance) -> Result<(), dispatch::DispatchError>  {
		Balances::<T>::mutate((token_id, to), |balance| *balance -= amount.clone());
		TotalSupply::<T>::mutate(token_id, |supply| *supply -= amount);

		Self::deposit_event(RawEvent::Burn(token_id, to.clone(), amount));

		Ok(())
	}

	pub fn burn_batch(token_id: T::TokenId, to: &Vec<T::AccountId>, amounts: Vec<T::TokenBalance>) -> Result<(), dispatch::DispatchError>  {
		for i in 0..to.len() {
			let account = &to[i];
			let amount = amounts[i];

			Balances::<T>::mutate((token_id, account), |balance| *balance -= amount.clone());
			TotalSupply::<T>::mutate(token_id, |supply| *supply -= amount);
		}

		Self::deposit_event(RawEvent::BurnBatch(token_id, to.clone(), amounts));
		
		Ok(())
	}

	pub fn do_safe_transfer_from(
		from: T::AccountId,
		to: T::AccountId,
		id: T::TokenId,
		amount: T::TokenBalance
	) -> dispatch::DispatchResult
	{
		let from_balance = Self::balance_of((id, from.clone()));
		ensure!(from_balance >= amount.clone(), Error::<T>::InsufficientFunds);

		Balances::<T>::mutate((id, from), |balance| *balance -= amount.clone());
		Balances::<T>::mutate((id, to), |balance| *balance += amount.clone());

		Ok(())
	}

}
