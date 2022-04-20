#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};
use cw2::set_contract_version;

use crate::coin_helper::{get_coin_u128, DENOM};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, StatusResponse};
use crate::state::{State, STATE};

// 合约版本信息，管理员可以用来对合约进行升级维护
const CONTRACT_NAME: &str = "crates.io:guess";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 升级合约
///
/// 对合约进行升级
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

/// 初始化
///
/// 通过初始化来创建合约
///
/// ## 参数
/// * DepsMut: 可以读写存储 `Storage`
/// * Env: 包含区块 `block` 和 合约信息 `contract`
/// * MessageInfo: 初始化操作 `MsgInstantiateContract` 和 执行操作 `MsgExecuteContract` 的附加信息， 包含 `sender` 和 `funds`
/// * InstantiateMsg: 自定义的初始化信息
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        guess_number: 0,
        owner: info.sender.clone(),
        guess_is_odd: false,
        bonus: Uint128::new(0),
        user_payed: Uint128::new(0),
        user: None,
        is_playing: false,
        is_lottery: false,
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

/// 执行操作
///
/// 对合约执行操作，修改合约信息
///
/// ## 参数
/// * DepsMut: 可以读写存储 `Storage`
/// * Env: 包含区块 `block` 和 合约信息 `contract`
/// * MessageInfo: 初始化操作 `MsgInstantiateContract` 和 执行操作 `MsgExecuteContract` 的附加信息， 包含 `sender` 和 `funds`
/// * ExecuteMsg: 自定义的信息
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Start {} => try_start(deps, info),
        ExecuteMsg::Reset { num } => try_reset(deps, info, num, env),
        ExecuteMsg::Guess { is_odd } => try_guess(deps, info, is_odd),
        ExecuteMsg::AddBonus {} => try_add_bonus(deps, info),
        ExecuteMsg::Lottery {} => try_lottery(deps, env),
    }
}

/// 开放投注
///
/// * 必须先开放投注，用户才可以下注猜大小
/// * 只能由合约管理员进行该操作
/// * 开放后不能重复开放，只能重置合约状态才能重新开放
pub fn try_start(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    // let balance = deps.querier.query_balance(env.contract.address, DENOM)?;
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        if state.owner != info.sender {
            return Err(ContractError::Unauthorized {});
        }
        if state.user.is_some() || state.is_playing {
            return Err(ContractError::IsPlaying {});
        }
        state.is_playing = true;
        Ok(state)
    })?;

    Ok(Response::new())
}

/// 用户下注
///
/// 用户下注并记录押的单还是双
/// * 下注的金额需要和奖金额一致
/// * 押大小后可以在开奖前修改单双
pub fn try_guess(
    deps: DepsMut,
    info: MessageInfo,
    is_odd: bool,
) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        let pay = get_coin_u128(&info);
        match state.user.as_ref() {
            Some(user) => {
                if user != &info.sender {
                    return Err(ContractError::Unauthorized {});
                }
            }
            None => state.user = Some(info.sender),
        }
        // 判断是否可押注和是否已开奖
        if !state.is_playing || state.is_lottery {
            return Err(ContractError::NotReady {});
        }
        // 判断下注金额是否和奖金一致
        if state.user_payed + pay != state.bonus {
            return Err(ContractError::Pay {});
        }
        state.user_payed += pay;
        state.guess_is_odd = is_odd;
        Ok(state)
    })?;

    Ok(Response::new())
}

/// 添加奖金
///
/// 管理员在开放押注前添加奖金，如果游戏已开始不能进行操作
pub fn try_add_bonus(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        if state.owner != info.sender {
            return Err(ContractError::Unauthorized {});
        }
        if state.user.is_some() || state.is_playing {
            return Err(ContractError::IsPlaying {});
        }
        let pay = get_coin_u128(&info);
        state.bonus += pay;
        Ok(state)
    })?;

    Ok(Response::new())
}

/// 开奖
///
/// 管理员和用户都可以进行开奖操作
/// 用户猜对，合约的奖金全部打到用户钱包
/// 用户猜错，合约的奖金全部打到管理员钱包
pub fn try_lottery(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let state = STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        if state.is_lottery || state.user.is_none() {
            return Err(ContractError::NotReady {});
        }
        state.is_lottery = true;
        state.is_playing = false;
        Ok(state)
    })?;

    let res = Response::new();

    let to_address = if state.guess_is_odd == (state.guess_number % 2 != 0) {
        state.owner.to_string()
    } else {
        state.user.as_ref().unwrap().to_string()
    };

    let balance = deps.querier.query_balance(env.contract.address, DENOM)?;

    Ok(res.add_message(BankMsg::Send {
        to_address,
        amount: vec![balance],
    }))
}

/// 重置游戏
///
/// 管理员可在游戏完成后重置游戏状态以进行下一轮游戏
pub fn try_reset(
    deps: DepsMut,
    info: MessageInfo,
    number: i8,
    env: Env,
) -> Result<Response, ContractError> {
    let state = STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        if state.is_playing && state.user.is_some() {
            return Err(ContractError::IsPlaying {});
        }
        if info.sender != state.owner {
            return Err(ContractError::Unauthorized {});
        }
        state.user = None;
        state.is_lottery = false;
        state.is_playing = false;
        state.guess_number = number;
        state.bonus = Uint128::new(0);
        Ok(state)
    })?;
    let balance = deps.querier.query_all_balances(env.contract.address)?;
    let mut res = Response::new();
    if !balance.is_empty() {
        res = res.add_message(BankMsg::Send {
            to_address: state.owner.to_string(),
            amount: balance,
        });
    }
    Ok(res)
}

/// 查询操作
///
/// 只对合约查询操作，无法修改合约信息
///
/// ## 参数
/// * Deps: 可以读存储 `Storage`
/// * Env: 包含区块 `block` 和 合约信息 `contract`
/// * QueryMsg: 自定义的信息
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetStatus {} => to_binary(&query_status(deps)?),
    }
}

/// 查询游戏状态
///
/// 返回游戏是否在进行和奖金金额
fn query_status(deps: Deps) -> StdResult<StatusResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(StatusResponse {
        playing: state.is_playing,
        bonus: state.bonus,
    })
}

// 用Mock模拟对合约进行单元测试
#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    // 测试初始化
    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(&coins(1000, DENOM));

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, DENOM));

        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetStatus {}).unwrap();
        let value: StatusResponse = from_binary(&res).unwrap();
        assert_eq!(0, value.bonus.u128());
    }

    #[test]
    fn set_bonus() {
        let mut deps = mock_dependencies(&coins(1000, DENOM));

        // 初始化合约
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(200, DENOM));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // 管理员设置奖金为200
        let info = mock_info("creator", &coins(200, DENOM));
        let msg = ExecuteMsg::AddBonus {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // 查询奖金是否200
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetStatus {}).unwrap();
        let value: StatusResponse = from_binary(&res).unwrap();
        assert_eq!(200, value.bonus.u128());
    }

    // 测试重置合约
    #[test]
    fn reset() {
        let mut deps = mock_dependencies(&coins(2000, DENOM));

        // 初始化合约
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(2, DENOM));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // 设置奖金为200
        let info = mock_info("creator", &coins(200, DENOM));
        let msg = ExecuteMsg::AddBonus {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // 测试奖金是否200
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetStatus {}).unwrap();
        let value: StatusResponse = from_binary(&res).unwrap();
        assert_eq!(200, value.bonus.u128());

        // 测试其他用户能否重置合约状态
        let unauth_info = mock_info("anyone", &coins(2, DENOM));
        let msg = ExecuteMsg::Reset { num: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // 管理员重置合约
        let auth_info = mock_info("creator", &[]);
        let msg = ExecuteMsg::Reset { num: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // 重置后奖金应为0
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetStatus {}).unwrap();
        let value: StatusResponse = from_binary(&res).unwrap();
        assert_eq!(0, value.bonus.u128());
    }
}
