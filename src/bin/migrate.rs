use deployer::{config::Database, storage::db};
use toasty_cli::{Config as MigrationConfig, ToastyCli};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    let config = Database::load()?;
    let mut args: Vec<String> = std::env::args().collect();
    let test_database = args.get(1).is_some_and(|arg| arg == "--test");

    if test_database {
        args.remove(1);
    }

    let db = db::connect(config.connection(test_database)?).await?;

    ToastyCli::with_config(db, MigrationConfig::load()?)
        .parse_from(args)
        .await?;

    Ok(())
}
