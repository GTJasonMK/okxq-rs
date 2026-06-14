use super::*;

mod builder;
mod counts;
mod entries;
mod health;
mod summary;
mod sync_records;

pub(in crate::commands::local_api::inventory) use self::builder::{
    build_inventory_payload, InventoryBuildOptions,
};
pub(in crate::commands::local_api) use self::health::market_data_health;
pub(in crate::commands::local_api) use self::sync_records::market_sync_records;

pub(crate) async fn market_inventory(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let include_storage_counts = param_bool(req, "include_storage_counts", false);
    Ok(code_ok(
        build_inventory_payload(
            state,
            InventoryBuildOptions {
                include_storage_counts,
                ..Default::default()
            },
        )
        .await?,
    ))
}
