use serde::Serialize;

#[derive(Serialize)]
pub struct AppInfo {
    name: String,
    version: String,
    platform: String,
    is_dev: bool,
}

mod formatting;
mod health;
mod info;
mod status;

pub use health::system_health;
pub use info::get_app_info;
pub use status::{system_status, system_status_value};
