use super::*;

pub(crate) fn agent_capabilities() -> Value {
    json!([
        {"name": "market-snapshot", "kind": "query", "goal": "获取指定币种的最新行情快照", "side_effect_free": true, "risk_level": "low"},
        {"name": "candles", "kind": "query", "goal": "获取指定币种的多周期K线数据", "side_effect_free": true, "risk_level": "low"},
        {"name": "indicators", "kind": "query", "goal": "获取指定币种的技术指标快照", "side_effect_free": true, "risk_level": "low"},
        {"name": "trading-context", "kind": "query", "goal": "聚合行情、盘口、成交、持仓等统一交易上下文", "side_effect_free": true, "risk_level": "low"},
        {"name": "watchlist-scan", "kind": "query", "goal": "遍历关注列表，对每个币种做快速信号评分", "side_effect_free": true, "risk_level": "low"},
        {"name": "data-health", "kind": "query", "goal": "统计每个币种的K线数据覆盖率与健康度", "side_effect_free": true, "risk_level": "low"},
        {"name": "orderbook", "kind": "query", "goal": "获取指定深度盘口快照", "side_effect_free": true, "risk_level": "low"},
        {"name": "recent-trades", "kind": "query", "goal": "获取逐笔成交记录", "side_effect_free": true, "risk_level": "low"},
        {"name": "position", "kind": "query", "goal": "获取当前持仓摘要", "side_effect_free": true, "risk_level": "low"},
        {"name": "multi-timeframe-alignment", "kind": "analysis", "goal": "多周期趋势一致性分析", "side_effect_free": true, "risk_level": "low"},
        {"name": "watchlist-correlation", "kind": "analysis", "goal": "关注列表币种价格相关性矩阵", "side_effect_free": true, "risk_level": "low"},
        {"name": "opportunity-patrol", "kind": "analysis", "goal": "扫描关注列表识别交易机会", "side_effect_free": true, "risk_level": "low"},
        {"name": "risk-budget", "kind": "analysis", "goal": "基于持仓和风险参数计算可用风险预算", "side_effect_free": true, "risk_level": "low"},
        {"name": "market-structure", "kind": "analysis", "goal": "分析K线摆动高/低点判断市场结构（趋势/盘整）", "side_effect_free": true, "risk_level": "low"},
        {"name": "support-resistance", "kind": "analysis", "goal": "从摆动点和成交量识别关键支撑阻力位", "side_effect_free": true, "risk_level": "low"},
        {"name": "price-projection", "kind": "analysis", "goal": "基于斐波那契/ATR/趋势回归的价格预测区间", "side_effect_free": true, "risk_level": "low"},
        {"name": "trade-setup", "kind": "analysis", "goal": "综合市场结构/SR/指标生成交易入场/止损/止盈建议", "side_effect_free": true, "risk_level": "low"},
        {"name": "python", "kind": "analysis", "goal": "在安全沙箱中执行 Python 代码片段并返回结果", "side_effect_free": false, "risk_level": "medium"}
    ])
}
