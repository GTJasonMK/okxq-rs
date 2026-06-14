use serde_json::{json, Value};

pub(in crate::commands::local_api::assistant) fn build_tools() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "get_market_snapshot",
                "description": "获取单个交易对的最新行情快照（最新价、涨跌幅、成交量等）",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "inst_id": {"type": "string", "description": "交易对，如 BTC-USDT"}
                    },
                    "required": ["inst_id"]
                }
            }
        }
        ,
        {
            "type": "function",
            "function": {
                "name": "get_candles",
                "description": "获取K线历史数据",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "inst_id": {"type": "string", "description": "交易对"},
                        "timeframe": {"type": "string", "description": "K线周期，如 15m/1H/4H/1D"},
                        "limit": {"type": "integer", "minimum": 20, "maximum": 300}
                    },
                    "required": ["inst_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_indicators",
                "description": "获取技术指标（MA/EMA/MACD/RSI/Bollinger/KDJ）",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "inst_id": {"type": "string", "description": "交易对"},
                        "timeframe": {"type": "string", "description": "K线周期"},
                        "limit": {"type": "integer", "minimum": 60, "maximum": 500}
                    },
                    "required": ["inst_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_trading_context",
                "description": "获取完整交易上下文（账户、持仓、订单、风险摘要）",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "inst_id": {"type": "string", "description": "交易对"},
                        "mode": {"type": "string", "enum": ["simulated", "live"]}
                    },
                    "required": ["inst_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "scan_watchlist",
                "description": "扫描关注币种列表，返回多维评分排序结果",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "mode": {"type": "string", "enum": ["simulated", "live"]}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_orderbook",
                "description": "获取订单簿深度数据（买卖盘口）",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "inst_id": {"type": "string", "description": "交易对"},
                        "depth": {"type": "integer", "minimum": 5, "maximum": 200}
                    },
                    "required": ["inst_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_recent_trades",
                "description": "获取最近的公开成交记录",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "inst_id": {"type": "string", "description": "交易对"},
                        "limit": {"type": "integer", "minimum": 5, "maximum": 100}
                    },
                    "required": ["inst_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_position",
                "description": "获取当前持仓详情",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "inst_id": {"type": "string", "description": "交易对（可选，不填返回所有持仓）"},
                        "mode": {"type": "string", "enum": ["simulated", "live"]}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "check_data_health",
                "description": "检查交易对的数据健康状态（K线完整性、延迟等）",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "inst_id": {"type": "string", "description": "交易对"}
                    },
                    "required": ["inst_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "analyze_multi_timeframe",
                "description": "多周期（15m/1H/4H）趋势一致性分析",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "inst_id": {"type": "string", "description": "交易对"}
                    },
                    "required": ["inst_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "analyze_correlation",
                "description": "计算关注列表币种间的价格相关性矩阵",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "timeframe": {"type": "string", "description": "K线周期"},
                        "lookback": {"type": "integer", "minimum": 50, "maximum": 500}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "patrol_opportunities",
                "description": "对关注币种进行机会巡检，返回候选机会和评分",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "inst_type": {"type": "string", "enum": ["SPOT", "SWAP"]},
                        "mode": {"type": "string", "enum": ["simulated", "live"]}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "calculate_risk_budget",
                "description": "基于账户权益和风险参数计算建议仓位和风险预算",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "inst_id": {"type": "string", "description": "交易对"},
                        "mode": {"type": "string", "enum": ["simulated", "live"]}
                    },
                    "required": ["inst_id"]
                }
            }
        }
    ])
}
