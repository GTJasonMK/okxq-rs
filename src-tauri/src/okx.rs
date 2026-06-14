mod client_order_id;
mod normalize;
mod outbound_record;
mod position_fields;
mod private;
mod public;
mod response;
mod retry;

pub(crate) use self::client_order_id::{
    generate_okx_client_order_id, normalized_okx_client_order_id,
};
pub(crate) use self::normalize::normalize_trade;
pub use self::normalize::{okx_bar, parse_okx_candle, OkxCandle};
pub(crate) use self::position_fields::{
    okx_account_equity, okx_finite_value, okx_position_notional, okx_position_side_dir,
    okx_position_side_label, okx_positive_value, okx_signed_position, okx_value_text,
};
pub(crate) use self::private::{
    normalized_order_type as normalized_okx_order_type,
    order_type_requires_price as okx_order_type_requires_price,
};
pub use self::private::{OkxAttachedAlgoOrder, OkxPrivateClient};
pub use self::public::OkxPublicClient;
