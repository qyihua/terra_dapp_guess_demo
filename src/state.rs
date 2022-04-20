use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::Item;

/// 合约状态信息
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// 管理员设置的数字
    pub guess_number: i8,

    /// 游戏是否在进行
    pub is_playing: bool,

    /// 是否已开奖
    pub is_lottery: bool,

    /// 用户猜的是否为单
    pub guess_is_odd: bool,

    /// 用户地址
    pub user: Option<Addr>,

    /// 管理员地址
    pub owner: Addr,

    /// 奖金大小
    pub bonus: Uint128,

    /// 用户已付金额
    pub user_payed: Uint128,
}

pub const STATE: Item<State> = Item::new("state");
