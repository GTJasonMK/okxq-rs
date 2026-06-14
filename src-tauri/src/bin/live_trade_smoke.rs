#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = okxq_rs_lib::live_trade_smoke::LiveTradeSmokeArgs::from_env_args(std::env::args())?;
    let report = okxq_rs_lib::live_trade_smoke::run_live_trade_smoke(args).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
