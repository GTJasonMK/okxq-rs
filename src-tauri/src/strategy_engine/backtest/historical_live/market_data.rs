use std::collections::{BTreeMap, HashMap};

use crate::okx::OkxCandle;

use super::values::{norm_symbol, norm_timeframe};

#[derive(Clone, Debug)]
pub struct HistoricalCandleSeries {
    pub symbol: String,
    pub inst_type: String,
    pub timeframe: String,
    pub candles: Vec<OkxCandle>,
}

#[derive(Clone, Debug)]
pub struct HistoricalFundingPoint {
    pub symbol: String,
    pub inst_type: String,
    pub funding_time: i64,
    pub funding_rate: f64,
}

#[derive(Clone, Debug)]
pub struct HistoricalFundingSeries {
    pub symbol: String,
    pub inst_type: String,
    pub rates: Vec<HistoricalFundingPoint>,
}

#[derive(Clone, Debug, Default)]
pub struct HistoricalMarketData {
    series: HashMap<(String, String), Vec<OkxCandle>>,
    funding: HashMap<(String, String), Vec<HistoricalFundingPoint>>,
}

impl HistoricalMarketData {
    pub fn new(series: Vec<HistoricalCandleSeries>) -> Self {
        let mut output = Self::default();
        for item in series {
            output.insert(item);
        }
        output
    }

    pub fn insert(&mut self, mut item: HistoricalCandleSeries) {
        let _inst_type = item.inst_type.trim();
        item.candles
            .retain(|candle| candle.is_valid_market_candle());
        let key = (norm_symbol(&item.symbol), norm_timeframe(&item.timeframe));
        let candles = self.series.entry(key).or_default();
        candles.extend(item.candles);
        *candles = candles_by_timestamp(candles.drain(..))
            .into_values()
            .collect();
    }

    pub fn with_funding(mut self, series: Vec<HistoricalFundingSeries>) -> Self {
        for item in series {
            self.insert_funding(item);
        }
        self
    }

    pub fn insert_funding(&mut self, mut item: HistoricalFundingSeries) {
        item.rates.retain(|point| {
            point.funding_time > 0
                && point.funding_rate.is_finite()
                && !point.symbol.trim().is_empty()
                && !point.inst_type.trim().is_empty()
        });
        item.rates.sort_by_key(|point| point.funding_time);
        let key = (norm_symbol(&item.symbol), norm_inst_type(&item.inst_type));
        let rates = self.funding.entry(key).or_default();
        rates.extend(item.rates);
        *rates = funding_by_timestamp(rates.drain(..))
            .into_values()
            .collect();
    }

    pub(super) fn next_candle_between(
        &self,
        symbol: &str,
        timeframe: &str,
        after_ts: i64,
        until_ts: i64,
    ) -> Option<OkxCandle> {
        self.series
            .get(&(norm_symbol(symbol), norm_timeframe(timeframe)))
            .and_then(|items| {
                items
                    .iter()
                    .find(|item| item.timestamp > after_ts && item.timestamp <= until_ts)
                    .cloned()
            })
    }

    pub(super) fn has_candle_series(&self, symbol: &str, timeframe: &str) -> bool {
        self.series
            .get(&(norm_symbol(symbol), norm_timeframe(timeframe)))
            .is_some_and(|items| !items.is_empty())
    }

    pub(super) fn last_close(&self, symbol: &str, timeframe: &str, timestamp: i64) -> Option<f64> {
        self.series
            .get(&(norm_symbol(symbol), norm_timeframe(timeframe)))
            .and_then(|items| {
                items
                    .iter()
                    .take_while(|item| item.timestamp <= timestamp)
                    .last()
                    .map(|item| item.close)
            })
            .filter(|value| value.is_finite() && *value > 0.0)
    }

    pub(super) fn has_funding_series(&self, symbol: &str, inst_type: &str) -> bool {
        self.funding
            .contains_key(&(norm_symbol(symbol), norm_inst_type(inst_type)))
    }

    pub(super) fn funding_between(
        &self,
        symbol: &str,
        inst_type: &str,
        after_ts: i64,
        until_ts: i64,
    ) -> Vec<HistoricalFundingPoint> {
        if until_ts <= after_ts {
            return Vec::new();
        }
        self.funding
            .get(&(norm_symbol(symbol), norm_inst_type(inst_type)))
            .map(|items| {
                items
                    .iter()
                    .filter(|item| item.funding_time > after_ts && item.funding_time <= until_ts)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn norm_inst_type(inst_type: &str) -> String {
    inst_type.trim().to_ascii_uppercase()
}

fn candles_by_timestamp(candles: impl IntoIterator<Item = OkxCandle>) -> BTreeMap<i64, OkxCandle> {
    candles
        .into_iter()
        .map(|candle| (candle.timestamp, candle))
        .collect()
}

fn funding_by_timestamp(
    rates: impl IntoIterator<Item = HistoricalFundingPoint>,
) -> BTreeMap<i64, HistoricalFundingPoint> {
    rates
        .into_iter()
        .map(|point| (point.funding_time, point))
        .collect()
}
