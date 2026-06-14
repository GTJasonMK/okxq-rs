use std::time::Duration;

use super::super::*;

pub(super) async fn pause_okx_page() {
    let pause_ms = okx_page_pause_ms();
    if pause_ms > 0 {
        tokio::time::sleep(Duration::from_millis(pause_ms)).await;
    }
}
