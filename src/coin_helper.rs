use cosmwasm_std::{Coin, MessageInfo, Uint128};

pub(crate) static DENOM: &str = "uluna";

pub fn get_coin_u128(info: &MessageInfo) -> Uint128 {
    match info.funds.as_slice() {
        [Coin { denom, amount }, ..] if denom == DENOM => *amount,
        _ => Uint128::new(0),
    }
}
