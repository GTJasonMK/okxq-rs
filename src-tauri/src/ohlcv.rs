#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ohlcv {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}
