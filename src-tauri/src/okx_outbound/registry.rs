use std::collections::HashMap;

use super::OKXOutboundRule;

const RULE_SPECS: &[(&str, &str, &str, &str, i64, i64)] = &[
    // 公开市场数据接口
    ("market.ticker", "public_ip", "rest", "public", 2, 20),
    ("market.tickers", "public_ip", "rest", "public", 2, 20),
    ("market.books", "public_ip", "rest", "public", 2, 40),
    ("market.books_full", "public_ip", "rest", "public", 2, 10),
    ("market.candles", "public_ip", "rest", "public", 2, 40),
    (
        "market.history_candles",
        "public_ip",
        "rest",
        "public",
        2,
        20,
    ),
    ("market.trades", "public_ip", "rest", "public", 2, 100),
    (
        "market.mark_price",
        "public_ip_inst",
        "rest",
        "public",
        2,
        10,
    ),
    ("market.index_ticker", "public_ip", "rest", "public", 2, 20),
    (
        "market.open_interest",
        "public_ip_inst",
        "rest",
        "public",
        2,
        20,
    ),
    (
        "market.funding_rate",
        "public_ip_inst",
        "rest",
        "public",
        2,
        10,
    ),
    ("public.instruments", "public_ip", "rest", "public", 2, 20),
    // 私有账户接口
    ("account.balance", "private_user", "rest", "private", 2, 10),
    (
        "account.positions",
        "private_user",
        "rest",
        "private",
        2,
        10,
    ),
    (
        "account.max_avail_size",
        "private_user",
        "rest",
        "private",
        2,
        20,
    ),
    ("account.max_size", "private_user", "rest", "private", 2, 20),
    ("account.config", "private_user", "rest", "private", 2, 5),
    (
        "account.set_position_mode",
        "private_user",
        "rest",
        "trade",
        2,
        5,
    ),
    (
        "account.set_leverage",
        "private_user",
        "rest",
        "trade",
        2,
        20,
    ),
    (
        "account.get_leverage",
        "private_user",
        "rest",
        "private",
        2,
        20,
    ),
    // 交易接口
    (
        "trade.order_detail",
        "trade_user_inst",
        "rest",
        "trade",
        2,
        60,
    ),
    (
        "trade.orders_pending",
        "private_user",
        "rest",
        "trade",
        2,
        60,
    ),
    (
        "trade.orders_history",
        "private_user",
        "rest",
        "trade",
        2,
        40,
    ),
    ("trade.fills", "private_user", "rest", "trade", 2, 60),
    (
        "trade.fills_history",
        "private_user",
        "rest",
        "trade",
        2,
        10,
    ),
    (
        "trade.place_order",
        "trade_user_inst",
        "rest",
        "trade",
        2,
        60,
    ),
    (
        "trade.cancel_order",
        "trade_user_inst",
        "rest",
        "trade",
        2,
        60,
    ),
    // WebSocket 连接控制
    ("ws.connect", "ws_connect_ip", "ws", "ws_control", 1, 3),
    ("ws.login", "ws_conn_ops", "ws", "ws_control", 3600, 480),
    ("ws.subscribe", "ws_conn_ops", "ws", "ws_control", 3600, 480),
    (
        "ws.unsubscribe",
        "ws_conn_ops",
        "ws",
        "ws_control",
        3600,
        480,
    ),
];

/// 限流规则注册表，包含 OKX 官方 API 的所有频率限制规则
pub struct OKXRateRuleRegistry {
    pub rules: HashMap<String, OKXOutboundRule>,
}

impl OKXRateRuleRegistry {
    pub fn new() -> Self {
        let rules = RULE_SPECS
            .iter()
            .map(
                |(op_key, rule_key, channel, target_group, window_seconds, capacity)| {
                    (
                        (*op_key).to_string(),
                        OKXOutboundRule {
                            op_key: (*op_key).to_string(),
                            rule_key: (*rule_key).to_string(),
                            channel: (*channel).to_string(),
                            target_group: (*target_group).to_string(),
                            window_seconds: *window_seconds,
                            capacity: *capacity,
                        },
                    )
                },
            )
            .collect();

        Self { rules }
    }

    pub fn get(&self, op_key: &str) -> Option<&OKXOutboundRule> {
        self.rules.get(op_key)
    }
}

impl Default for OKXRateRuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}
