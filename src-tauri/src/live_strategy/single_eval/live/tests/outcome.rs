use super::*;

#[test]
fn retryable_submit_failures_release_dedupe_key_for_retry() {
    let transient = LiveActionExecutionOutcome::retryable("提交 OKX 订单失败: connection reset");

    assert!(
        transient.should_release_action_dedupe_key(),
        "retryable submit failures should be retried on the same action key"
    );
}

#[test]
fn terminal_submit_failures_keep_dedupe_key() {
    let invalid_size = LiveActionExecutionOutcome::terminal(
        "策略显式 exchange_size=0.0001 小于 OKX minSz，已拒绝下单以避免静默改量",
    );
    let invalid_close_side =
        LiveActionExecutionOutcome::terminal("无法根据平仓订单方向 flat 推导 OKX 目标持仓方向");
    let missing_identity =
        LiveActionExecutionOutcome::terminal("cancel_order 动作缺少可撤订单身份");

    assert!(
        !invalid_size.should_release_action_dedupe_key(),
        "terminal open validation failures should not loop on the same action"
    );
    assert!(
        !invalid_close_side.should_release_action_dedupe_key(),
        "terminal close contract failures should not be retried blindly"
    );
    assert!(
        !missing_identity.should_release_action_dedupe_key(),
        "terminal order-management contract failures should not be retried blindly"
    );
}

#[test]
fn close_resolution_no_position_validation_is_retryable() {
    let no_position =
        AppError::Validation("OKX 当前没有 BTC-USDT-SWAP 可平持仓，已拒绝实时策略平仓".to_string());
    let no_position_reason = no_position.to_string();
    assert!(
        retryable_for_close_resolution_error(&no_position, &no_position_reason),
        "close_position should retry when OKX position/fill state may still be syncing"
    );

    let invalid_close_side =
        AppError::Validation("无法根据平仓订单方向 flat 推导 OKX 目标持仓方向".to_string());
    let invalid_close_side_reason = invalid_close_side.to_string();
    assert!(
        !retryable_for_close_resolution_error(&invalid_close_side, &invalid_close_side_reason),
        "invalid close_position contract fields must remain terminal"
    );
}
