use crate::{
    config::ApiCredentials,
    error::{AppError, AppResult},
    live_strategy::{runtime_helpers::canonical_timeframe, types::LiveStrategyConfig},
    realtime::RealtimeManager,
    strategy_executor,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::live_strategy::runtime::lifecycle) enum LivePrivateSubscription {
    Account,
    Orders,
    AlgoOrders,
    Fills,
    Positions,
}

pub(in crate::live_strategy::runtime::lifecycle) const LIVE_PRIVATE_SUBSCRIPTIONS:
    [LivePrivateSubscription; 5] = [
    LivePrivateSubscription::Account,
    LivePrivateSubscription::Orders,
    LivePrivateSubscription::AlgoOrders,
    LivePrivateSubscription::Fills,
    LivePrivateSubscription::Positions,
];

impl LivePrivateSubscription {
    pub(in crate::live_strategy::runtime::lifecycle) async fn subscribe(
        self,
        realtime: &RealtimeManager,
        mode: &str,
        credentials: ApiCredentials,
    ) -> AppResult<()> {
        match self {
            Self::Account => realtime.subscribe_account(mode, credentials).await,
            Self::Orders => realtime.subscribe_orders(mode, credentials).await,
            Self::AlgoOrders => realtime.subscribe_algo_orders(mode, credentials).await,
            Self::Fills => realtime.subscribe_fills(mode, credentials).await,
            Self::Positions => realtime.subscribe_positions(mode, credentials).await,
        }
        .map(|_| ())
    }

    pub(in crate::live_strategy::runtime::lifecycle) async fn unsubscribe(
        self,
        realtime: &RealtimeManager,
        mode: &str,
    ) -> AppResult<()> {
        match self {
            Self::Account => realtime.unsubscribe_account(mode).await,
            Self::Orders => realtime.unsubscribe_orders(mode).await,
            Self::AlgoOrders => realtime.unsubscribe_algo_orders(mode).await,
            Self::Fills => realtime.unsubscribe_fills(mode).await,
            Self::Positions => realtime.unsubscribe_positions(mode).await,
        }
        .map(|_| ())
    }

    pub(in crate::live_strategy::runtime::lifecycle) fn channel_key(self) -> &'static str {
        match self {
            Self::Account => "account",
            Self::Orders => "orders",
            Self::AlgoOrders => "orders-algo",
            Self::Fills => "fills",
            Self::Positions => "positions",
        }
    }

    pub(in crate::live_strategy::runtime::lifecycle) fn label(self) -> &'static str {
        match self {
            Self::Account => "账户",
            Self::Orders => "订单",
            Self::AlgoOrders => "保护单",
            Self::Fills => "成交",
            Self::Positions => "仓位",
        }
    }
}

pub(in crate::live_strategy::runtime::lifecycle) fn live_candle_subscriptions(
    config: &LiveStrategyConfig,
    data_requirements: &serde_json::Value,
) -> AppResult<Vec<(String, String)>> {
    let requirements = strategy_executor::candle_requirements(
        data_requirements,
        &config.symbol,
        &config.inst_type,
        &config.timeframe,
        3,
    );
    let mut subscriptions = Vec::new();
    for requirement in requirements {
        let symbol = strategy_executor::normalize_runtime_inst_id(
            &requirement.symbol,
            &requirement.inst_type,
        )?;
        let timeframe = canonical_timeframe(&requirement.timeframe)
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "{} {} DATA_REQUIREMENTS 声明了不支持的 K 线周期: {}",
                    symbol,
                    requirement.inst_type,
                    requirement.timeframe.trim()
                ))
            })?
            .to_string();
        if !subscriptions.iter().any(|(known_symbol, known_timeframe)| {
            known_symbol == &symbol && known_timeframe == &timeframe
        }) {
            subscriptions.push((symbol, timeframe));
        }
    }
    Ok(subscriptions)
}

pub(in crate::live_strategy::runtime::lifecycle) fn live_trigger_subscriptions(
    config: &LiveStrategyConfig,
    subscriptions: &[(String, String)],
) -> AppResult<Vec<(String, String)>> {
    let trigger_symbol =
        strategy_executor::normalize_runtime_inst_id(&config.symbol, &config.inst_type)?;
    let trigger_timeframe = canonical_timeframe(&config.timeframe)
        .ok_or_else(|| {
            AppError::Validation(format!(
                "不支持的实时策略触发 K 线周期: {}",
                config.timeframe.trim()
            ))
        })?
        .to_string();
    let primary = subscriptions
        .iter()
        .find(|(symbol, timeframe)| {
            symbol == &trigger_symbol && timeframe.eq_ignore_ascii_case(&trigger_timeframe)
        })
        .cloned();
    if let Some(primary) = primary {
        return Ok(vec![primary]);
    }
    let same_timeframe = subscriptions
        .iter()
        .filter(|(_, timeframe)| timeframe.eq_ignore_ascii_case(&trigger_timeframe))
        .cloned()
        .collect::<Vec<_>>();
    if !same_timeframe.is_empty() {
        return Ok(same_timeframe);
    }
    Ok(subscriptions.to_vec())
}

pub(in crate::live_strategy::runtime::lifecycle) fn has_portfolio_layers(
    params: &serde_json::Value,
) -> bool {
    params
        .as_object()
        .is_some_and(|object| object.contains_key("portfolio_layers"))
}

pub(in crate::live_strategy::runtime::lifecycle) fn portfolio_layers_rejection_message(
) -> &'static str {
    "实时策略已删除 portfolio_layers 本地组合架构，请使用交易所单策略执行"
}
