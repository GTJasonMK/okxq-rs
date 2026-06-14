use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
};

use zip::ZipArchive;

use crate::okx::OkxCandle;

use super::repair_plan::{expected_candles_in_range, historical_zip_aggr_type, GapRange};
use super::*;

const HISTORICAL_CANDLESTICKS_MODULE: &str = "2";

#[derive(Clone, Debug, Default)]
pub(super) struct HistoricalZipImportReport {
    pub(super) files: i64,
    pub(super) links: i64,
    pub(super) rows_seen: i64,
    pub(super) rows_imported: i64,
    pub(super) saved_count: i64,
}

pub(super) async fn import_okx_historical_zip_range(
    context: CandleFetchContext<'_>,
    start_ts: i64,
    end_ts: i64,
) -> AppResult<HistoricalZipImportReport> {
    if context.timeframe != BASE_CANDLE_TIMEFRAME {
        return Err(AppError::Validation(format!(
            "OKX historical zip 当前仅作为 {} 基础数据来源，收到 timeframe={}",
            BASE_CANDLE_TIMEFRAME, context.timeframe
        )));
    }
    let date_aggr_type = historical_zip_aggr_type(&GapRange {
        start_ts,
        end_ts,
        missing_candles: expected_candles_in_range(
            start_ts,
            end_ts,
            timeframe_to_ms(BASE_CANDLE_TIMEFRAME),
        ),
    });
    let links = context
        .client
        .get_historical_market_data_links(
            HISTORICAL_CANDLESTICKS_MODULE,
            context.inst_type,
            &[context.inst_id.to_string()],
            start_ts,
            end_ts,
            date_aggr_type,
        )
        .await?;
    if links.is_empty() {
        return Err(AppError::Runtime(format!(
            "OKX historical zip 未返回下载文件：{} {} {} 至 {}",
            context.inst_id,
            context.inst_type,
            ts_to_iso(Some(start_ts)).as_str().unwrap_or(""),
            ts_to_iso(Some(end_ts)).as_str().unwrap_or("")
        )));
    }

    let mut report = HistoricalZipImportReport {
        links: links.len() as i64,
        ..Default::default()
    };
    for link in links {
        check_sync_cancel(context.cancel_guard).await?;
        let zip_bytes = context.client.download_historical_zip(&link.url).await?;
        check_sync_cancel(context.cancel_guard).await?;
        let parsed = parse_okx_candlestick_zip(&zip_bytes, context.inst_id, start_ts, end_ts)?;
        report.files += 1;
        report.rows_seen += parsed.rows_seen;
        report.rows_imported += parsed.candles.len() as i64;
        if parsed.candles.is_empty() {
            tracing::warn!(
                inst_id = context.inst_id,
                inst_type = context.inst_type,
                filename = %link.filename,
                url = %link.url,
                date_ts = link.date_ts,
                size_mb = link.size_mb,
                "OKX historical zip contained no matching candles for requested range"
            );
            continue;
        }
        let saved = save_candles(
            CandleSaveScope {
                pool: context.pool,
                inst_id: context.inst_id,
                inst_type: context.inst_type,
                timeframe: BASE_CANDLE_TIMEFRAME,
                cancel_guard: context.cancel_guard,
                progress: context.progress,
            },
            &parsed.candles,
            CandleSaveProgress {
                fetched_count: report.rows_imported,
                target_fetch_count: report.rows_imported,
                target_save_count: report.rows_imported,
                batches: report.links,
                target_batches: report.links,
                api_calls: report.links,
                saved_offset: report.saved_count,
                derived_offset: 0,
                target_derive_count: 0,
                derived: false,
            },
        )
        .await?;
        report.saved_count += saved;
    }
    Ok(report)
}

#[derive(Clone, Debug, Default)]
struct ParsedHistoricalCandles {
    rows_seen: i64,
    candles: Vec<OkxCandle>,
}

fn parse_okx_candlestick_zip(
    zip_bytes: &[u8],
    inst_id: &str,
    start_ts: i64,
    end_ts: i64,
) -> AppResult<ParsedHistoricalCandles> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|error| AppError::Runtime(format!("历史 zip 解析失败: {error}")))?;
    let mut rows_seen = 0i64;
    let mut candles = BTreeMap::<i64, OkxCandle>::new();
    let normalized_inst_id = inst_id.trim().to_uppercase();
    let mut csv_members = 0i64;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| AppError::Runtime(format!("历史 zip 成员读取失败: {error}")))?;
        let name = file.name().to_string();
        if !name.to_ascii_lowercase().ends_with(".csv") {
            continue;
        }
        csv_members += 1;
        let mut content = Vec::new();
        file.read_to_end(&mut content)
            .map_err(|error| AppError::Runtime(format!("历史 zip CSV 读取失败: {error}")))?;
        let mut reader = csv::ReaderBuilder::new()
            .flexible(true)
            .from_reader(Cursor::new(content));
        let headers = reader
            .headers()
            .map_err(|error| AppError::Runtime(format!("历史 zip CSV 表头读取失败: {error}")))?
            .clone();
        let columns = HistoricalCandleColumns::from_headers(&headers)?;
        for record in reader.records() {
            let record = record
                .map_err(|error| AppError::Runtime(format!("历史 zip CSV 行读取失败: {error}")))?;
            rows_seen += 1;
            let Some(candle) = parse_historical_candle_record(
                &record,
                &columns,
                &normalized_inst_id,
                start_ts,
                end_ts,
            )?
            else {
                continue;
            };
            candles.insert(candle.timestamp, candle);
        }
    }
    if csv_members <= 0 {
        return Err(AppError::Runtime("历史 zip 中没有 CSV 文件".to_string()));
    }
    Ok(ParsedHistoricalCandles {
        rows_seen,
        candles: candles.into_values().collect(),
    })
}

#[derive(Clone, Debug)]
struct HistoricalCandleColumns {
    inst_id: usize,
    timestamp: usize,
    open: usize,
    high: usize,
    low: usize,
    close: usize,
    volume: usize,
    volume_ccy: usize,
    volume_quote: usize,
}

impl HistoricalCandleColumns {
    fn from_headers(headers: &csv::StringRecord) -> AppResult<Self> {
        Ok(Self {
            inst_id: required_csv_index(headers, "instrument_name")?,
            timestamp: required_csv_index(headers, "open_time")?,
            open: required_csv_index(headers, "open")?,
            high: required_csv_index(headers, "high")?,
            low: required_csv_index(headers, "low")?,
            close: required_csv_index(headers, "close")?,
            volume: required_csv_index(headers, "vol")?,
            volume_ccy: required_csv_index(headers, "vol_ccy")?,
            volume_quote: required_any_csv_index(
                headers,
                &["vol_ccy_quote", "volCcyQuote", "volume_quote", "vol_quote"],
            )?,
        })
    }
}

fn parse_historical_candle_record(
    record: &csv::StringRecord,
    columns: &HistoricalCandleColumns,
    inst_id: &str,
    start_ts: i64,
    end_ts: i64,
) -> AppResult<Option<OkxCandle>> {
    let row_inst_id = record
        .get(columns.inst_id)
        .unwrap_or_default()
        .trim()
        .to_uppercase();
    if row_inst_id != inst_id {
        return Ok(None);
    }
    let timestamp = parse_csv_i64(record, columns.timestamp, "open_time")?;
    if timestamp < start_ts || timestamp > end_ts {
        return Ok(None);
    }
    Ok(Some(OkxCandle {
        timestamp,
        open: parse_csv_f64(record, columns.open, "open")?,
        high: parse_csv_f64(record, columns.high, "high")?,
        low: parse_csv_f64(record, columns.low, "low")?,
        close: parse_csv_f64(record, columns.close, "close")?,
        volume: parse_csv_f64(record, columns.volume, "vol")?,
        volume_ccy: parse_csv_f64(record, columns.volume_ccy, "vol_ccy")?,
        volume_quote: parse_csv_f64(record, columns.volume_quote, "vol_ccy_quote")?,
        confirm: "1".to_string(),
    }))
}

fn required_csv_index(headers: &csv::StringRecord, name: &str) -> AppResult<usize> {
    optional_csv_index(headers, name)
        .ok_or_else(|| AppError::Runtime(format!("历史 zip CSV 缺少字段 {name}")))
}

fn required_any_csv_index(headers: &csv::StringRecord, names: &[&str]) -> AppResult<usize> {
    names
        .iter()
        .find_map(|name| optional_csv_index(headers, name))
        .ok_or_else(|| AppError::Runtime(format!("历史 zip CSV 缺少字段 {}", names.join("/"))))
}

fn optional_csv_index(headers: &csv::StringRecord, name: &str) -> Option<usize> {
    headers
        .iter()
        .position(|header| header.trim().eq_ignore_ascii_case(name))
}

fn parse_csv_i64(record: &csv::StringRecord, index: usize, field: &str) -> AppResult<i64> {
    record
        .get(index)
        .unwrap_or_default()
        .trim()
        .parse::<i64>()
        .map_err(|error| AppError::Runtime(format!("历史 zip CSV 字段 {field} 解析失败: {error}")))
}

fn parse_csv_f64(record: &csv::StringRecord, index: usize, field: &str) -> AppResult<f64> {
    let raw = record.get(index).unwrap_or_default().trim();
    let value = raw.parse::<f64>().map_err(|error| {
        AppError::Runtime(format!("历史 zip CSV 字段 {field} 解析失败: {error}"))
    })?;
    if !value.is_finite() {
        return Err(AppError::Runtime(format!(
            "历史 zip CSV 字段 {field} 解析失败: 非有限数字 {raw}"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::*;

    fn test_zip(name: &str, text: &str) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut archive = zip::ZipWriter::new(cursor);
        archive
            .start_file(name, SimpleFileOptions::default())
            .expect("start zip csv");
        archive.write_all(text.as_bytes()).expect("write zip csv");
        archive.finish().expect("finish zip").into_inner()
    }

    #[test]
    fn historical_zip_parser_reads_okx_candlestick_csv_schema() {
        let zip = test_zip(
            "BTC-USDT-SWAP-1m.csv",
            "instrument_name,open_time,open,high,low,close,vol,vol_ccy,vol_ccy_quote\n\
             BTC-USDT-SWAP,1704067200000,1,2,0.5,1.5,10,100,101\n\
             ETH-USDT-SWAP,1704067200000,1,2,0.5,1.5,10,100,101\n\
             BTC-USDT-SWAP,1704067260000,1.5,2.5,1,2,20,200,202\n",
        );

        let parsed =
            parse_okx_candlestick_zip(&zip, "BTC-USDT-SWAP", 1_704_067_200_000, 1_704_067_260_000)
                .expect("parse okx historical zip");

        assert_eq!(parsed.rows_seen, 3);
        assert_eq!(parsed.candles.len(), 2);
        assert_eq!(parsed.candles[0].timestamp, 1_704_067_200_000);
        assert_eq!(parsed.candles[0].volume_ccy, 100.0);
        assert_eq!(parsed.candles[0].volume_quote, 101.0);
        assert_eq!(parsed.candles[1].close, 2.0);
    }

    #[test]
    fn historical_zip_parser_rejects_dirty_required_volume_fields() {
        let zip = test_zip(
            "ETH-USDT-SWAP-1m.csv",
            "instrument_name,open_time,open,high,low,close,vol,vol_ccy,vol_ccy_quote\n\
             ETH-USDT-SWAP,1704067200000,1,2,0.5,1.5,10,,100\n",
        );

        let err =
            parse_okx_candlestick_zip(&zip, "ETH-USDT-SWAP", 1_704_067_200_000, 1_704_067_320_000)
                .expect_err("dirty volume fields must fail import");

        assert!(err.to_string().contains("字段 vol_ccy 解析失败"));
    }

    #[test]
    fn historical_zip_parser_rejects_dirty_required_price_fields() {
        let zip = test_zip(
            "ETH-USDT-SWAP-1m.csv",
            "instrument_name,open_time,open,high,low,close,vol,vol_ccy,vol_ccy_quote\n\
             ETH-USDT-SWAP,1704067200000,bad,2,0.5,1.5,10,100,101\n",
        );

        let err =
            parse_okx_candlestick_zip(&zip, "ETH-USDT-SWAP", 1_704_067_200_000, 1_704_067_200_000)
                .expect_err("bad price fields must fail import");

        assert!(err.to_string().contains("字段 open 解析失败"));
    }
}
