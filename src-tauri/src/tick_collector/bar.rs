use serde_json::Value;

/// 秒级聚合柱。
#[derive(Clone, Debug, Default)]
pub(super) struct SecondBar {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    count: i64,
    mid_price: f64,
    mid_count: i64,
    bid_price: f64,
    ask_price: f64,
    spread_bps: f64,
    book_count: i64,
}

impl SecondBar {
    pub(super) fn ingest_trade(&mut self, price: f64, size: f64) {
        if !price.is_finite() || price <= 0.0 || !size.is_finite() || size <= 0.0 {
            return;
        }
        if self.count == 0 {
            self.open = price;
            self.high = price;
            self.low = price;
        }
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        self.volume += size;
        self.count += 1;
    }

    pub(super) fn ingest_book(&mut self, bid: f64, ask: f64) {
        if !bid.is_finite() || !ask.is_finite() || bid <= 0.0 || ask <= 0.0 || ask < bid {
            return;
        }
        let mid = (bid + ask) / 2.0;
        self.ingest_mid(mid);
        self.bid_price =
            (self.bid_price * self.book_count as f64 + bid) / (self.book_count as f64 + 1.0);
        self.ask_price =
            (self.ask_price * self.book_count as f64 + ask) / (self.book_count as f64 + 1.0);
        let spread_bps = ((ask - bid) / mid) * 10_000.0;
        self.spread_bps = (self.spread_bps * self.book_count as f64 + spread_bps)
            / (self.book_count as f64 + 1.0);
        self.book_count += 1;
    }

    pub(super) fn has_trade(&self) -> bool {
        self.count > 0
    }

    pub(super) fn to_payload_json(&self) -> Option<Value> {
        if !self.has_trade() {
            return None;
        }

        Some(serde_json::json!({
            "open": self.open,
            "high": self.high,
            "low": self.low,
            "close": self.close,
            "volume": self.volume,
            "trade_count": self.count,
            "mid_price": self.mid_price,
            "mid_count": self.mid_count,
            "bid_price": self.bid_price,
            "ask_price": self.ask_price,
            "spread_bps": self.spread_bps,
            "book_count": self.book_count,
        }))
    }

    fn ingest_mid(&mut self, mid: f64) {
        self.mid_price =
            (self.mid_price * self.mid_count as f64 + mid) / (self.mid_count as f64 + 1.0);
        self.mid_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_collector_bar_does_not_emit_quote_only_ohlc() {
        let mut bar = SecondBar::default();
        bar.ingest_book(100.0, 101.0);

        assert!(bar.to_payload_json().is_none());
    }

    #[test]
    fn tick_collector_bar_emits_trade_ohlc_with_quote_features() {
        let mut bar = SecondBar::default();
        bar.ingest_book(100.0, 101.0);
        bar.ingest_trade(100.5, 2.0);

        let payload = bar
            .to_payload_json()
            .expect("trade bar should emit payload");
        assert_eq!(payload["open"], 100.5);
        assert_eq!(payload["high"], 100.5);
        assert_eq!(payload["low"], 100.5);
        assert_eq!(payload["close"], 100.5);
        assert_eq!(payload["volume"], 2.0);
        assert_eq!(payload["trade_count"], 1);
        assert_eq!(payload["mid_count"], 1);
    }

    #[test]
    fn tick_collector_bar_rejects_invalid_trade_ohlc_inputs() {
        let mut bar = SecondBar::default();
        bar.ingest_trade(0.0, 1.0);
        bar.ingest_trade(100.0, 0.0);

        assert!(
            bar.to_payload_json().is_none(),
            "invalid trade inputs should not create an OHLCV bar"
        );
    }

    #[test]
    fn tick_collector_bar_rejects_non_finite_orderbook_mid_inputs() {
        let mut bar = SecondBar::default();
        bar.ingest_book(f64::NAN, 101.0);
        bar.ingest_trade(100.0, 1.0);

        let payload = bar
            .to_payload_json()
            .expect("valid trade should emit payload");
        assert_eq!(payload["mid_count"], 0);
        assert_eq!(payload["mid_price"], 0.0);
    }
}
