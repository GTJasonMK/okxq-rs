use sqlx::{Row, SqlitePool};

use crate::{
    error::{AppError, AppResult},
    live_strategy::runtime_helpers::canonical_timeframe,
};

use super::LEGACY_DERIVATION_SOURCE_TIMEFRAME;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum TimeframeStorageKind {
    Direct,
    Derived { source_timeframe: String },
}

pub(super) async fn resolve_timeframe_storage_kind_from_db(
    db: &SqlitePool,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
) -> AppResult<TimeframeStorageKind> {
    let Some(timeframe) = canonical_timeframe(timeframe).map(|value| value.to_string()) else {
        return Ok(TimeframeStorageKind::Direct);
    };
    if let Some(kind) =
        resolve_timeframe_storage_kind_from_sync_jobs(db, symbol, inst_type, &timeframe).await?
    {
        return Ok(kind);
    }
    if let Some(kind) =
        resolve_timeframe_storage_kind_from_sync_records(db, symbol, inst_type, &timeframe).await?
    {
        return Ok(kind);
    }
    Ok(TimeframeStorageKind::Direct)
}

async fn resolve_timeframe_storage_kind_from_sync_jobs(
    db: &SqlitePool,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
) -> AppResult<Option<TimeframeStorageKind>> {
    let rows = sqlx::query(
        r#"
        SELECT timeframe, source_timeframe, target_timeframes
        FROM sync_jobs
        WHERE inst_id = ? AND inst_type = ? AND LOWER(status) = 'completed'
        ORDER BY updated_at DESC, created_at DESC
        LIMIT 200
        "#,
    )
    .bind(symbol)
    .bind(inst_type)
    .fetch_all(db)
    .await?;

    for row in rows {
        let display_timeframe = row.try_get::<String, _>("timeframe")?;
        let source_timeframe = row.try_get::<String, _>("source_timeframe")?;
        let target_timeframes = row.try_get::<String, _>("target_timeframes")?;
        if !sync_job_mentions_timeframe(&display_timeframe, &target_timeframes, timeframe)? {
            continue;
        }

        let Some(source_timeframe) =
            canonical_timeframe(&source_timeframe).map(|value| value.to_string())
        else {
            return Err(AppError::Validation(format!(
                "同步任务记录的源 K 线周期无效：{}",
                source_timeframe.trim()
            )));
        };
        if source_timeframe == timeframe {
            return Ok(Some(TimeframeStorageKind::Direct));
        }
        return Ok(Some(TimeframeStorageKind::Derived { source_timeframe }));
    }

    Ok(None)
}

async fn resolve_timeframe_storage_kind_from_sync_records(
    db: &SqlitePool,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
) -> AppResult<Option<TimeframeStorageKind>> {
    let mode = sqlx::query_scalar::<_, String>(
        r#"
        SELECT last_sync_mode
        FROM sync_records
        WHERE inst_id = ? AND inst_type = ? AND timeframe = ?
        LIMIT 1
        "#,
    )
    .bind(symbol)
    .bind(inst_type)
    .bind(timeframe)
    .fetch_optional(db)
    .await?;

    if mode.is_some_and(|value| value.eq_ignore_ascii_case("derive")) {
        return Ok(Some(TimeframeStorageKind::Derived {
            source_timeframe: LEGACY_DERIVATION_SOURCE_TIMEFRAME.to_string(),
        }));
    }
    Ok(None)
}

fn sync_job_mentions_timeframe(
    display_timeframe: &str,
    raw_target_timeframes: &str,
    timeframe: &str,
) -> AppResult<bool> {
    if canonical_timeframe(display_timeframe).is_some_and(|value| value == timeframe) {
        return Ok(true);
    }
    let target_timeframes = serde_json::from_str::<Vec<String>>(raw_target_timeframes)?;
    for target in target_timeframes {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(normalized) = canonical_timeframe(trimmed) else {
            return Err(AppError::Validation(format!(
                "同步任务记录的目标 K 线周期无效：{}",
                trimmed
            )));
        };
        if normalized == timeframe {
            return Ok(true);
        }
    }
    Ok(false)
}
