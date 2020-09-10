use crate::{Trait, Error, Module};
use sp_runtime::traits::{
    One, Zero,
};
use frame_support::{
	ensure, dispatch,
	traits::{Currency, ExistenceRequirement},
};

pub fn collateral_join<T: Trait>(token_id: T::TokenId, sender: T::AccountId, account: T::AccountId, amount: T::TokenBalance) -> dispatch::DispatchResult {
	ensure!(amount >= Zero::zero(), Error::<T>::InsufficientAmount);

	let module_account = Module::<T>::account_id();

	cdp::Module::<T>::increase_collateral(token_id, account, amount);
	token::Module::<T>::do_safe_transfer_from(&token_id, &sender, &module_account, amount);

	Ok(())
}

pub fn collateral_exit<T: Trait>(token_id: T::TokenId, sender: T::AccountId, account: T::AccountId, amount: T::TokenBalance) {
	let module_account = Module::<T>::account_id();
	cdp::Module::<T>::decrease_collateral(token_id, sender, amount);
	token::Module::<T>::do_safe_transfer_from(&token_id, &module_account, &account, amount);
}

pub fn bei_join<T: Trait>(sender: T::AccountId, account: T::AccountId, amount: T::TokenBalance) {
	let module_account = Module::<T>::account_id();
	let bei_token_id = Module::<T>::bei_token_id();

	cdp::Module::<T>::transfer_bei(module_account, account, amount);
	token::Module::<T>::burn(&bei_token_id, &sender, amount);
}

pub fn bei_exit<T: Trait>(sender: T::AccountId, account: T::AccountId, amount: T::TokenBalance) {
	let module_account = Module::<T>::account_id();
	let bei_token_id = Module::<T>::bei_token_id();

	cdp::Module::<T>::transfer_bei(sender, module_account, amount);
	token::Module::<T>::mint(&bei_token_id, &account, amount);
}
