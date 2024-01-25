use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::U128;
use near_sdk::{
    assert_one_yocto, assert_self, env, ext_contract, near_bindgen, AccountId, Gas, PanicOnDefault,
    Promise, PromiseOrValue,
};

const FEES_ACCOUNT_ID: &str = "fees.test.near";
const FT_BALANCE_GAS: Gas = Gas(3_000_000_000_000);
const CALCULATE_FEES_GAS: Gas = Gas(5_000_000_000_000);
const FT_TRANSFER_GAS: Gas = Gas(10_000_000_000_000);
const FT_TRANSFER_CALL_GAS: Gas = Gas(30_000_000_000_000);
const CALCULATE_FEES_CALLBACK_GAS: Gas = Gas(30_000_000_000_000);
const FINISH_FORWARD_GAS: Gas = Gas(30_000_000_000_000);

#[near_bindgen]
#[derive(Debug, BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct AuroraForwarder {
    target_address: String,
    target_network: AccountId,
    fees_contract_id: AccountId,
    owner: AccountId,
}

#[near_bindgen]
impl AuroraForwarder {
    #[must_use]
    #[init]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(target_address: String, target_network: AccountId) -> Self {
        let owner = env::predecessor_account_id();
        let target_address = target_address.trim_start_matches("0x").to_string();
        let fees_contract_id = AccountId::new_unchecked(FEES_ACCOUNT_ID.to_string());

        Self {
            target_address,
            target_network,
            fees_contract_id,
            owner,
        }
    }

    #[payable]
    pub fn forward(&mut self, token_id: AccountId) -> Promise {
        assert_one_yocto();

        ext_token::ext(token_id.clone())
            .with_static_gas(FT_BALANCE_GAS)
            .ft_balance_of(env::current_account_id())
            .then(
                ext_self::ext(env::current_account_id())
                    .with_attached_deposit(env::attached_deposit())
                    .with_static_gas(CALCULATE_FEES_CALLBACK_GAS)
                    .calculate_fees_callback(token_id),
            )
    }

    #[payable]
    pub fn calculate_fees_callback(
        &mut self,
        #[callback] amount: U128,
        token_id: &AccountId,
    ) -> Promise {
        assert_self();

        ext_fees::ext(self.fees_contract_id.clone())
            .with_static_gas(CALCULATE_FEES_GAS)
            .calculate_fees(amount, token_id, &self.target_network)
            .then(
                ext_self::ext(env::current_account_id())
                    .with_attached_deposit(2)
                    .with_static_gas(FINISH_FORWARD_GAS)
                    .finish_forward_callback(amount, token_id),
            )
    }

    #[payable]
    pub fn finish_forward_callback(
        &mut self,
        #[callback] fee: U128,
        amount: U128,
        token_id: AccountId,
    ) -> Promise {
        assert_self();
        let amount = U128::from(amount.0.saturating_sub(fee.0));

        ext_token::ext(token_id.clone())
            .with_attached_deposit(near_sdk::ONE_YOCTO)
            .with_static_gas(FT_TRANSFER_CALL_GAS)
            .ft_transfer_call(
                self.target_network.clone(),
                amount,
                None,
                self.target_address.clone(),
            )
            .then(
                ext_token::ext(token_id)
                    .with_attached_deposit(near_sdk::ONE_YOCTO)
                    .with_static_gas(FT_TRANSFER_GAS)
                    .ft_transfer(self.fees_contract_id.clone(), fee),
            )
    }
}

#[ext_contract(ext_self)]
pub trait ExtAuroraForwarder {
    fn calculate_fees_callback(&mut self, #[callback] amount: U128, token_id: AccountId)
        -> Promise;

    fn finish_forward_callback(
        &mut self,
        #[callback] fee: U128,
        amount: U128,
        token_id: &AccountId,
    ) -> Promise;
}

#[ext_contract(ext_token)]
pub trait ExtToken {
    fn ft_balance_of(&self, account_id: AccountId) -> U128;

    fn ft_transfer(&self, receiver_id: AccountId, amount: U128);

    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128>;
}

#[ext_contract(ext_fees)]
pub trait ExtFeesCalculator {
    fn calculate_fees(
        &self,
        amount: U128,
        token_id: &AccountId,
        target_network: &AccountId,
    ) -> U128;
}