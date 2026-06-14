use std::collections::BTreeMap;

use super::RealtimeState;

#[derive(Clone, Copy)]
pub(in crate::realtime) enum PrivateChannel {
    Account,
    Orders,
    AlgoOrders,
    Fills,
    Positions,
}

pub(in crate::realtime) fn private_refs_mut(
    state: &mut RealtimeState,
    channel: PrivateChannel,
) -> &mut BTreeMap<String, usize> {
    match channel {
        PrivateChannel::Account => &mut state.private_account_refs,
        PrivateChannel::Orders => &mut state.private_order_refs,
        PrivateChannel::AlgoOrders => &mut state.private_algo_order_refs,
        PrivateChannel::Fills => &mut state.private_fill_refs,
        PrivateChannel::Positions => &mut state.private_position_refs,
    }
}

pub(in crate::realtime) fn has_public_subscriptions(state: &RealtimeState) -> bool {
    !state.ticker_refs.is_empty()
        || !state.trade_refs.is_empty()
        || !state.orderbook_refs.is_empty()
}

pub(in crate::realtime) fn has_private_subscriptions_for_mode(
    state: &RealtimeState,
    mode: &str,
) -> bool {
    state.private_account_refs.contains_key(mode)
        || state.private_order_refs.contains_key(mode)
        || state.private_fill_refs.contains_key(mode)
        || state.private_position_refs.contains_key(mode)
}

pub(in crate::realtime) fn has_private_business_subscriptions_for_mode(
    state: &RealtimeState,
    mode: &str,
) -> bool {
    state.private_algo_order_refs.contains_key(mode)
}

pub(in crate::realtime) fn has_any_private_subscriptions_for_mode(
    state: &RealtimeState,
    mode: &str,
) -> bool {
    has_private_subscriptions_for_mode(state, mode)
        || has_private_business_subscriptions_for_mode(state, mode)
}
