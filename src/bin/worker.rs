#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    deployer::init_logging();
    let config = deployer::config::Config::load()?;
    let store = deployer::storage::Store::connect(&config.database.url).await?;

    let current_provider = deployer::providers::current_provider(&config.providers)?;
    let mailer = deployer::mail::Transport::new(config.mail.as_ref())?;

    deployer::worker::run(store, current_provider, config.worker, mailer).await
}
