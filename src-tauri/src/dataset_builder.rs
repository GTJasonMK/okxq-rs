//! 研究平台数据集构建管线 — 从 feature_bars_1s 秒级K线构建结构化训练数据集。
//!
//! 流程：
//!   1. 从 feature_bars_1s 读取指定币种的秒级 OHLCV 序列
//!   2. 逐行计算滚动特征（收益/均线偏离/波动率/量比等）
//!   3. 生成多时间尺度的前向收益标签（1分钟/5分钟/15分钟）
//!   4. 按时间顺序切分为训练/验证/测试集（60/20/20）
//!   5. 写入 research_dataset_splits 表
//!   6. 创建/更新 research_dataset_manifests 清单
//!

use serde_json::{json, Value};
use sqlx::SqlitePool;

mod features;
mod loader;
mod storage;

use self::features::{build_features, build_labels, FEATURE_NAMES, LABEL_NAMES};
use self::loader::load_raw_bars;
pub use self::storage::get_dataset_splits;
use self::storage::{write_manifest, write_split};

type RawBar = (i64, f64, f64, f64, f64, f64, f64);

/// 数据集构建配置。
#[derive(Clone, Debug)]
pub struct DatasetBuildConfig {
    pub inst_id: String,
    pub inst_type: String,
    pub timeframe: String,
    pub bar_count: i64,
    pub dataset_id: Option<String>,
    /// 滚动特征窗口大小（秒）
    pub window_5: usize,
    pub window_20: usize,
}

impl Default for DatasetBuildConfig {
    fn default() -> Self {
        Self {
            inst_id: "BTC-USDT".to_string(),
            inst_type: "SPOT".to_string(),
            timeframe: "1H".to_string(),
            bar_count: 3600,
            dataset_id: None,
            window_5: 5,
            window_20: 20,
        }
    }
}

/// 构建数据集：读取原始数据 → 计算特征 → 生成标签 → 切分 → 写入数据库。
pub async fn build_dataset(db: &SqlitePool, config: &DatasetBuildConfig) -> Result<Value, String> {
    // 1. 读取原始秒级K线
    let raw = load_raw_bars(
        db,
        &config.inst_id,
        &config.inst_type,
        &config.timeframe,
        config.bar_count,
    )
    .await
    .map_err(|e| format!("读取 feature bars 失败: {e}"))?;

    let min_rows = 120;
    if raw.len() < min_rows {
        return Err(format!(
            "数据不足：需要至少{min_rows}根bar，当前仅{}根",
            raw.len()
        ));
    }

    // 2. 逐行构建特征
    let features = build_features(&raw, config);

    // 3. 生成标签
    let rows = build_labels(features);

    // 标记可用的行（既有特征又有标签）
    let labeled: Vec<_> = rows.iter().filter(|r| r.label_1m.is_some()).collect();
    let total_labeled = labeled.len();
    if total_labeled < 30 {
        return Err(format!("有效样本不足：仅{}行有标签", total_labeled));
    }

    // 4. 按时间顺序切分（60/20/20）
    let train_end = (total_labeled as f64 * 0.6).round() as usize;
    let val_end = (total_labeled as f64 * 0.8).round() as usize;

    // 5. 生成 dataset_id
    let dataset_id = config
        .dataset_id
        .clone()
        .unwrap_or_else(|| format!("ds_{}", chrono::Utc::now().timestamp_millis()));

    let now = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;

    // 6. 计算统计摘要
    let all_labels: Vec<f64> = labeled.iter().filter_map(|r| r.label_1m).collect();
    let label_mean = all_labels.iter().sum::<f64>() / all_labels.len() as f64;
    let label_std = (all_labels
        .iter()
        .map(|v| (v - label_mean) * (v - label_mean))
        .sum::<f64>()
        / all_labels.len() as f64)
        .sqrt();
    let pos_ratio =
        all_labels.iter().filter(|v| **v > 0.0).count() as f64 / all_labels.len() as f64;
    if !label_mean.is_finite() || !label_std.is_finite() || !pos_ratio.is_finite() {
        return Err("标签统计不是有限数字".to_string());
    }

    let summary = json!({
        "dataset_id": dataset_id,
        "inst_id": config.inst_id,
        "inst_type": config.inst_type,
        "timeframe": config.timeframe,
        "total_bar_count": raw.len(),
        "labeled_sample_count": total_labeled,
        "train_count": train_end,
        "val_count": val_end - train_end,
        "test_count": total_labeled - val_end,
        "feature_count": FEATURE_NAMES.len(),
        "label_count": LABEL_NAMES.len(),
        "feature_names": FEATURE_NAMES,
        "label_names": LABEL_NAMES,
        "split_ratio": "60/20/20",
        "label_mean_1m": round6(label_mean),
        "label_std_1m": round6(label_std),
        "label_positive_ratio_1m": round6(pos_ratio),
        "bar_resolution_sec": 1,
    });

    // 7. 写入 split 和 manifest。统计先算完，避免事务持有数据库写锁过久。
    let mut tx = db
        .begin()
        .await
        .map_err(|e| format!("开始数据集写入事务失败: {e}"))?;
    write_split(&mut tx, &dataset_id, "train", &labeled[..train_end], 0, now).await?;
    write_split(
        &mut tx,
        &dataset_id,
        "val",
        &labeled[train_end..val_end],
        train_end as i64,
        now,
    )
    .await?;
    write_split(
        &mut tx,
        &dataset_id,
        "test",
        &labeled[val_end..],
        val_end as i64,
        now,
    )
    .await?;
    write_manifest(&mut tx, &dataset_id, &summary, now).await?;
    tx.commit()
        .await
        .map_err(|e| format!("提交数据集写入事务失败: {e}"))?;

    Ok(summary)
}

fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}
