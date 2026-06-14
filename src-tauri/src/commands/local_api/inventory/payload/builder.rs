use super::counts::{
    apply_cost_basis_counts, apply_count_table, apply_deletion_mark_residual_counts,
    apply_local_fills_counts,
};
use super::entries::{ensure_inventory_entry, ensure_inventory_market};
use super::summary::finalize_inventory_payload;
use super::sync_records::apply_sync_record_rows;
use super::*;
use crate::commands::local_api::inventory::deletion::deletion_marked_symbols;

#[derive(Clone, Debug, Default)]
pub(in crate::commands::local_api::inventory) struct InventoryBuildOptions {
    /// The default inventory path treats sync_records as the K-line metadata cache.
    /// Candle writers must keep that table current so UI refreshes do not scan candles.
    pub(in crate::commands::local_api::inventory) include_storage_counts: bool,
    pub(in crate::commands::local_api::inventory) symbol_filter: Option<String>,
}

pub(in crate::commands::local_api::inventory) async fn build_inventory_payload(
    state: &AppState,
    options: InventoryBuildOptions,
) -> AppResult<Value> {
    let watched_items = state.preferences.watched_symbols().await?;
    let symbol_filter = options.symbol_filter.as_deref().and_then(normalize_symbol);
    let watched_symbols = watched_items
        .iter()
        .filter(|item| {
            symbol_filter
                .as_deref()
                .map_or(true, |symbol| item.symbol == symbol)
        })
        .map(|item| item.symbol.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let managed_scopes = enabled_scopes_from_watched(&watched_items)
        .into_iter()
        .filter(|scope| {
            symbol_filter
                .as_deref()
                .map_or(true, |symbol| scope.symbol == symbol)
        })
        .collect::<Vec<_>>();
    let managed_scope_keys = scope_keys(&managed_scopes);
    let mut entries: BTreeMap<String, Value> = BTreeMap::new();

    for item in &watched_items {
        if symbol_filter
            .as_deref()
            .is_some_and(|symbol| item.symbol != symbol)
        {
            continue;
        }
        let entry = ensure_inventory_entry(&mut entries, &item.symbol);
        if let Some(obj) = entry.as_object_mut() {
            obj.insert("base_ccy".to_string(), Value::String(item.base_ccy.clone()));
            obj.insert(
                "spot_inst_id".to_string(),
                Value::String(item.spot_inst_id.clone()),
            );
            obj.insert(
                "swap_inst_id".to_string(),
                Value::String(item.swap_inst_id.clone()),
            );
        }
    }

    for scope in &managed_scopes {
        let entry = ensure_inventory_entry(&mut entries, &scope.symbol);
        ensure_inventory_market(entry, &scope.inst_type, &scope.inst_id, true, true);
    }

    apply_sync_record_rows(
        state,
        &mut entries,
        &watched_symbols,
        &managed_scope_keys,
        !options.include_storage_counts,
        symbol_filter.as_deref(),
    )
    .await?;
    if options.include_storage_counts {
        apply_storage_counts(state, &mut entries).await?;
    }
    let mut deletion_marks = deletion_marked_symbols(&state.db).await?;
    if let Some(symbol) = symbol_filter.as_deref() {
        deletion_marks.retain(|item| item == symbol);
    }
    apply_deletion_mark_residual_counts(&state.db, &mut entries, &deletion_marks).await?;

    Ok(finalize_inventory_payload(
        entries,
        &watched_symbols,
        managed_scopes.len(),
        &deletion_marks,
    ))
}

async fn apply_storage_counts(
    state: &AppState,
    entries: &mut BTreeMap<String, Value>,
) -> AppResult<()> {
    apply_count_table(&state.db, entries, "candles", "inst_id", "candles").await?;
    apply_count_table(
        &state.db,
        entries,
        "feature_bars_1s",
        "inst_id",
        "feature_bars_1s",
    )
    .await?;
    apply_count_table(
        &state.db,
        entries,
        "sync_records",
        "inst_id",
        "sync_records",
    )
    .await?;
    apply_count_table(
        &state.db,
        entries,
        "market_ticker_snapshots",
        "inst_id",
        "market_ticker_snapshots",
    )
    .await?;
    apply_count_table(
        &state.db,
        entries,
        "market_recent_trades",
        "inst_id",
        "market_recent_trades",
    )
    .await?;
    apply_count_table(
        &state.db,
        entries,
        "live_order_records",
        "symbol",
        "live_order_records",
    )
    .await?;
    apply_count_table(
        &state.db,
        entries,
        "backtest_results",
        "symbol",
        "backtest_results",
    )
    .await?;
    apply_cost_basis_counts(&state.db, entries).await?;
    apply_local_fills_counts(&state.db, entries).await
}
