use crate::trading_semantics::order_type_omits_price;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ActionSubmissionKeyInput<'a> {
    pub(crate) symbol: &'a str,
    pub(crate) action: &'a str,
    pub(crate) order_type: &'a str,
    pub(crate) action_side: &'a str,
    pub(crate) action_price: f64,
    pub(crate) action_position_size: Option<f64>,
    pub(crate) action_timestamp: i64,
    pub(crate) order_side: Option<&'a str>,
    pub(crate) exchange_size: Option<&'a str>,
    pub(crate) planned_exit_timestamp: Option<i64>,
    pub(crate) action_identity: Option<&'a str>,
    pub(crate) action_index: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ActionDedupeIdentityInput<'a> {
    pub(crate) action: &'a str,
    pub(crate) cancel_order: Option<OrderManagementCancelIdentity<'a>>,
    pub(crate) modify_order: Option<OrderManagementModifyIdentity<'a>>,
    pub(crate) stop_loss: Option<f64>,
    pub(crate) take_profit: Option<f64>,
    pub(crate) max_slippage: Option<f64>,
    pub(crate) attached_risk_orders: &'a [RiskOrderIdentity<'a>],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OrderManagementCancelIdentity<'a> {
    pub(crate) target_kind: &'a str,
    pub(crate) order_id: &'a str,
    pub(crate) client_order_id: &'a str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OrderManagementModifyIdentity<'a> {
    pub(crate) target_kind: &'a str,
    pub(crate) target_order_type: Option<&'a str>,
    pub(crate) order_id: &'a str,
    pub(crate) client_order_id: &'a str,
    pub(crate) new_size: Option<&'a str>,
    pub(crate) new_price: Option<&'a str>,
    pub(crate) cancel_on_fail: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RiskOrderIdentity<'a> {
    pub(crate) symbol: &'a str,
    pub(crate) side: &'a str,
    pub(crate) order_type: &'a str,
    pub(crate) trigger_price: Option<f64>,
    pub(crate) stop_loss: Option<f64>,
    pub(crate) take_profit: Option<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OrderManagementTargetClass {
    Exchange,
    Algo,
}

pub(crate) fn action_submission_key(input: ActionSubmissionKeyInput<'_>) -> String {
    let action_index_key =
        action_index_submission_key(input.action, input.action_identity, input.action_index);
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        normalize_key_text(input.symbol).to_ascii_uppercase(),
        input.action_timestamp,
        normalize_action_name(input.action),
        normalize_key_text(input.order_type),
        normalize_key_text(input.action_side),
        normalize_key_order_price(input.order_type, input.action_price),
        input
            .action_position_size
            .map(normalize_key_number)
            .unwrap_or_default(),
        input.order_side.map(normalize_key_text).unwrap_or_default(),
        input
            .exchange_size
            .map(normalize_key_text)
            .unwrap_or_default(),
        input.planned_exit_timestamp.unwrap_or_default(),
        input
            .action_identity
            .map(normalize_key_text)
            .unwrap_or_default(),
        action_index_key
    )
}

pub(crate) fn action_dedupe_identity(input: ActionDedupeIdentityInput<'_>) -> Option<String> {
    if let Some(identity) = order_management_action_identity(input.cancel_order, input.modify_order)
    {
        return Some(identity);
    }
    match normalize_action_name(input.action).as_str() {
        "open_position" => open_risk_action_identity(
            input.stop_loss,
            input.take_profit,
            input.max_slippage,
            input.attached_risk_orders,
        ),
        "place_risk_order" => standalone_risk_action_identity(input.attached_risk_orders),
        _ => None,
    }
}

pub(crate) fn order_management_action_identity(
    cancel_order: Option<OrderManagementCancelIdentity<'_>>,
    modify_order: Option<OrderManagementModifyIdentity<'_>>,
) -> Option<String> {
    if let Some(cancel) = cancel_order {
        return Some(format!(
            "cancel|kind={}|ord={}|cl={}",
            normalize_key_text(cancel.target_kind),
            cancel.order_id.trim(),
            cancel.client_order_id.trim()
        ));
    }
    modify_order.map(|modify| {
        format!(
            "modify|kind={}|target_type={}|ord={}|cl={}|new_size={}|new_price={}|cancel_on_fail={}",
            normalize_key_text(modify.target_kind),
            normalize_key_text(modify.target_order_type.unwrap_or_default()),
            modify.order_id.trim(),
            modify.client_order_id.trim(),
            normalize_order_management_decimal(modify.new_size.unwrap_or_default()),
            normalize_order_management_decimal(modify.new_price.unwrap_or_default()),
            modify.cancel_on_fail,
        )
    })
}

pub(crate) fn order_management_identity_matches(
    order_id: &str,
    client_order_id: &str,
    target_order_id: &str,
    target_client_order_id: &str,
) -> bool {
    (!target_order_id.trim().is_empty() && order_id == target_order_id)
        || (!target_client_order_id.trim().is_empty() && client_order_id == target_client_order_id)
}

pub(crate) fn order_management_scope_matches(
    order_symbol: &str,
    action_symbol: &str,
    scope_explicit: bool,
) -> bool {
    !scope_explicit || order_symbol.eq_ignore_ascii_case(action_symbol)
}

pub(crate) fn order_management_target_kind_allows_class(
    target_kind: &str,
    target_class: OrderManagementTargetClass,
) -> bool {
    match normalize_key_text(target_kind).as_str() {
        "any" => true,
        "exchange" => matches!(target_class, OrderManagementTargetClass::Exchange),
        "algo" => matches!(target_class, OrderManagementTargetClass::Algo),
        _ => false,
    }
}

fn action_index_submission_key(
    action: &str,
    action_identity: Option<&str>,
    action_index: usize,
) -> String {
    if action_identity.is_some() && normalize_action_name(action) != "open_position" {
        return String::new();
    }
    action_index.to_string()
}

fn open_risk_action_identity(
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    max_slippage: Option<f64>,
    attached_risk_orders: &[RiskOrderIdentity<'_>],
) -> Option<String> {
    let risk_settings = risk_settings_identity(stop_loss, take_profit, max_slippage);
    let attached = attached_risk_orders_identity(attached_risk_orders);
    if risk_settings.is_empty() && attached.is_empty() {
        None
    } else {
        Some(format!("open_risk|{risk_settings}|attached={attached}"))
    }
}

fn standalone_risk_action_identity(
    attached_risk_orders: &[RiskOrderIdentity<'_>],
) -> Option<String> {
    let attached = attached_risk_orders_identity(attached_risk_orders);
    if attached.is_empty() {
        None
    } else {
        Some(format!("place_risk_order|attached={attached}"))
    }
}

fn risk_settings_identity(
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    max_slippage: Option<f64>,
) -> String {
    if stop_loss.is_none() && take_profit.is_none() && max_slippage.is_none() {
        return String::new();
    }
    format!(
        "sl={}|tp={}|slip={}",
        normalize_optional_identity_number(stop_loss),
        normalize_optional_identity_number(take_profit),
        normalize_optional_identity_number(max_slippage)
    )
}

fn attached_risk_orders_identity(attached_risk_orders: &[RiskOrderIdentity<'_>]) -> String {
    attached_risk_orders
        .iter()
        .map(|risk| {
            format!(
                "sym={}|side={}|type={}|trigger={}|sl={}|tp={}",
                normalize_identity_text(risk.symbol).to_ascii_uppercase(),
                normalize_identity_text(risk.side),
                normalize_identity_text(risk.order_type),
                normalize_optional_identity_number(risk.trigger_price),
                normalize_optional_identity_number(risk.stop_loss),
                normalize_optional_identity_number(risk.take_profit)
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn normalize_action_name(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn normalize_order_management_decimal(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }
    match value.parse::<f64>() {
        Ok(parsed) if parsed.is_finite() => {
            let formatted = format!("{parsed:.12}");
            formatted
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        }
        _ => value.to_string(),
    }
}

fn normalize_optional_identity_number(value: Option<f64>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    if !value.is_finite() {
        return String::new();
    }
    let formatted = format!("{value:.12}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn normalize_identity_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn normalize_key_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn normalize_key_number(value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let formatted = format!("{value:.12}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn normalize_key_order_price(order_type: &str, price: f64) -> String {
    if order_type_omits_price(order_type) {
        String::new()
    } else {
        normalize_key_number(price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_management_identity_normalizes_modify_decimals_and_target_kind() {
        let first = OrderManagementModifyIdentity {
            target_kind: "Algo",
            target_order_type: Some("stop_market"),
            order_id: "ord-1",
            client_order_id: "cl-1",
            new_size: Some("2.0000"),
            new_price: Some("101.250000"),
            cancel_on_fail: false,
        };
        let second = OrderManagementModifyIdentity {
            new_size: Some("2"),
            new_price: Some("101.25"),
            ..first
        };

        assert_eq!(
            order_management_action_identity(None, Some(first)),
            order_management_action_identity(None, Some(second))
        );
        assert_eq!(
            order_management_action_identity(None, Some(first)).as_deref(),
            Some("modify|kind=algo|target_type=stop_market|ord=ord-1|cl=cl-1|new_size=2|new_price=101.25|cancel_on_fail=false")
        );
    }

    #[test]
    fn action_identity_includes_open_and_standalone_risk_details() {
        let risk_orders = [RiskOrderIdentity {
            symbol: "BTC-USDT-SWAP",
            side: "sell",
            order_type: "stop_market",
            trigger_price: Some(94.0),
            stop_loss: Some(0.06),
            take_profit: None,
        }];

        assert_eq!(
            action_dedupe_identity(ActionDedupeIdentityInput {
                action: "open_position",
                cancel_order: None,
                modify_order: None,
                stop_loss: Some(0.06),
                take_profit: None,
                max_slippage: Some(0.002),
                attached_risk_orders: &risk_orders,
            })
            .as_deref(),
            Some("open_risk|sl=0.06|tp=|slip=0.002|attached=sym=BTC-USDT-SWAP|side=sell|type=stop_market|trigger=94|sl=0.06|tp=")
        );
        assert_eq!(
            action_dedupe_identity(ActionDedupeIdentityInput {
                action: "place_risk_order",
                cancel_order: None,
                modify_order: None,
                stop_loss: None,
                take_profit: None,
                max_slippage: None,
                attached_risk_orders: &risk_orders,
            })
            .as_deref(),
            Some("place_risk_order|attached=sym=BTC-USDT-SWAP|side=sell|type=stop_market|trigger=94|sl=0.06|tp=")
        );
    }

    #[test]
    fn action_submission_key_omits_market_price_and_ignores_index_for_identified_non_open() {
        let key = action_submission_key(ActionSubmissionKeyInput {
            symbol: "btc-usdt-swap",
            action: "cancel_order",
            order_type: "market",
            action_side: "buy",
            action_price: 100.0,
            action_position_size: Some(0.2),
            action_timestamp: 123,
            order_side: Some("sell"),
            exchange_size: Some("2"),
            planned_exit_timestamp: Some(456),
            action_identity: Some("cancel|ord=1"),
            action_index: 9,
        });

        assert_eq!(
            key,
            "BTC-USDT-SWAP|123|cancel_order|market|buy||0.2|sell|2|456|cancel|ord=1|"
        );
    }

    #[test]
    fn order_management_target_matching_rules_are_shared() {
        assert!(order_management_identity_matches(
            "ord-1", "cl-1", "ord-1", ""
        ));
        assert!(order_management_identity_matches(
            "ord-1", "cl-1", "", "cl-1"
        ));
        assert!(!order_management_identity_matches("ord-1", "cl-1", "", ""));
        assert!(order_management_scope_matches(
            "BTC-USDT-SWAP",
            "btc-usdt-swap",
            true
        ));
        assert!(!order_management_scope_matches(
            "BTC-USDT-SWAP",
            "ETH-USDT-SWAP",
            true
        ));
        assert!(order_management_scope_matches(
            "BTC-USDT-SWAP",
            "ETH-USDT-SWAP",
            false
        ));
        assert!(order_management_target_kind_allows_class(
            "any",
            OrderManagementTargetClass::Exchange
        ));
        assert!(order_management_target_kind_allows_class(
            "algo",
            OrderManagementTargetClass::Algo
        ));
        assert!(!order_management_target_kind_allows_class(
            "exchange",
            OrderManagementTargetClass::Algo
        ));
    }
}
