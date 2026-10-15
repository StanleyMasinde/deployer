static MIGRATIONS: toasty::migration::MigrationSet = toasty::embed_migrations!("toasty");

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    let command = std::env::args().nth(1);
    match command.as_deref() {
        Some("--version") => {
            println!("{}", deployer::VERSION);
            return Ok(());
        }
        Some("check-config") => {
            let config = deployer::config::Config::load()?;
            deployer::providers::current_provider(&config.providers)?;
            deployer::mail::Transport::new(config.mail.as_ref())?;
            return Ok(());
        }
        Some("ready") => {
            let config = deployer::config::Config::load()?;
            let address = config.server.listen;
            let host = if address.ip().is_unspecified() {
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
            } else {
                address.ip()
            };
            let endpoint = format!(
                "http://{}/health/readiness",
                std::net::SocketAddr::new(host, address.port())
            );
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()?;

            for _ in 0..60 {
                if client
                    .get(&endpoint)
                    .send()
                    .await
                    .is_ok_and(|response| response.status().is_success())
                {
                    return Ok(());
                }

                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }

            anyhow::bail!("Deployer did not become ready at {endpoint}");
        }
        Some("--help") => {
            println!(
                "deployer [--version | --help | check-config | ready]\n\nStarts the web server. Set DEPLOYER_CONFIG to the TOML config path.\ncheck-config validates configuration and GitHub key access.\nready waits for the HTTP readiness endpoint."
            );
            return Ok(());
        }
        Some(_) => anyhow::bail!("unknown command; use --help"),
        None => {}
    }

    deployer::init_logging();
    deployer::http::run(deployer::config::Config::load()?, &MIGRATIONS).await
}
