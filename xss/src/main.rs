mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = std::env::args().collect::<Vec<_>>();
    let config = config::Config::from_path(opt.get(1).unwrap_or(&"config.toml".into())).expect("Failed to load config");
    let _g = ss_utils::logs::enable_log(&config.log_config)?;
    Ok(())
}
