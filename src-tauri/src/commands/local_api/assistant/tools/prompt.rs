use serde_json::Value;

pub(in crate::commands::local_api::assistant) fn build_system_prompt(
    market_context: &Value,
) -> String {
    let ctx_text =
        serde_json::to_string(market_context).expect("assistant market context should serialize");
    format!(
        "\
你是 OKX 量化交易桌面应用里的分析助手。你能够调用工具函数获取实时行情数据、技术指标、成交记录等。
回答要基于工具返回的实际数据，明确不确定性，不要声称已经执行真实下单。
不要编造数据，所有数字必须来自工具返回结果。
当用户询问行情时，优先调用相关工具获取最新数据。

当前行情上下文 JSON: {ctx_text}"
    )
}
