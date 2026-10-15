# Contributing to Deployer

Use this guide to run Deployer locally and check your changes before opening a pull request. For VPS installation, see the [README](README.md).

## Prerequisites

Install these tools before you start:

- Rust stable with Cargo, rustfmt, and Clippy.
- MySQL 8.4 or later, running locally.
- Node.js 24 or later and pnpm 12.8.1, as specified in [frontend/package.json](frontend/package.json).
- OpenSSL to generate secrets.

You also need a GitHub App client ID, installation ID, and private key. Follow [Create your GitHub App](README.md#create-your-github-app) to obtain them.

## Configure your local environment

Run commands from the repository root unless stated otherwise.

1. Install the frontend dependencies:

   ```sh
   pnpm --dir frontend install --frozen-lockfile
   ```

2. Create a dedicated local MySQL database named `deployer`. Give your local database user permission to create and alter tables in it; the web server applies migrations on startup.

3. If you don't have a local configuration, copy the example:

   ```sh
   cp config.toml.example config.toml
   ```

   Keep your existing `config.toml` if you have already configured it.

4. Create writable local directories:

   ```sh
   mkdir -p .local/state .local/assets .local/apps
   ```

5. Edit the following settings in `config.toml`, preserving the other settings from the example:

   | Setting | Local value |
   | --- | --- |
   | `server.listen` | `"127.0.0.1:8080"` |
   | `server.public_url` | `"http://127.0.0.1:8080"` |
   | `server.assets_root` | `"./.local/assets"` |
   | `server.state_root` | `"./.local/state"` |
   | `database.url` | `"mysql://DB_USER:DB_PASSWORD@127.0.0.1:3306/deployer"` |
   | `worker.apps_root` | `"./.local/apps"` |

   Replace `DB_USER` and `DB_PASSWORD` with your local database credentials. URL-encode special characters in the credentials.

6. Set `providers.github.app_id` to your GitHub App **client ID**, `installation_id` to its numeric installation ID, and `private_key` to the absolute path of its readable PEM file.

7. Generate independent values for `server.session_secret` and `providers.github.webhook_secret`. Run this command once for each secret:

   ```sh
   openssl rand -hex 32
   ```

`config.toml` and `.local/` are ignored by Git. Keep private keys outside tracked files.

## Start the web server

Validate your configuration and start the server:

```sh
cargo run -- check-config
cargo run
```

The debug server connects to MySQL, applies embedded migrations, creates a setup code if no root account exists, and starts Vite automatically on port 5173. Use the application through **http://127.0.0.1:8080**, where the backend serves API requests and proxies frontend requests to Vite.

Open **http://127.0.0.1:8080/setup** and create your root account. Read the setup code in another terminal:

```sh
cat .local/state/setup-code
```

After setup, use **http://127.0.0.1:8080/console**. The setup code is removed after you create the account.

Verify that the server is ready:

```sh
curl -fsS http://127.0.0.1:8080/health/readiness
```

The response contains `"status":"ready"`. Stop the server with Ctrl+C.

> [!NOTE]
> Use the default debug build for local development. Release builds require HTTPS and versioned frontend assets.

## Process deployments

After the web server has started and applied migrations, you can start the worker in another terminal:

```sh
cargo run --bin deployer-worker
```

Publishing apps requires a Linux environment with Nginx, systemd, and the privileged helper configured by the installer. Starting the local web server on macOS does not configure that publishing environment. The helper generates app configurations at `/etc/nginx/conf.d/deployer-APP_ID.conf` and serves files from `/var/lib/deployer/apps/APP_ID/current`.

To receive GitHub events locally, expose port 8080 through a public HTTPS tunnel. In your development GitHub App, set the webhook URL to `https://TUNNEL_HOST/api/webhooks/github`, use the same webhook secret as your local config, and subscribe to **Workflow run** events. Replace `TUNNEL_HOST` with your tunnel's hostname.

## Troubleshoot startup

| Symptom | What to check |
| --- | --- |
| `Origin does not match this instance` | Open `http://127.0.0.1:8080`, matching `server.public_url` exactly. The scheme, hostname, and port must match; `localhost` and `127.0.0.1` are different origins. Do not open the application through Vite on port 5173. If you prefer `http://localhost:8080`, set `server.public_url` to that value and restart `cargo run`. |
| `Permission denied` during startup | Set `server.state_root` to `./.local/state`. Production paths under `/var/lib/deployer` might not be writable by your local user. |
| Database connection failure | Start MySQL and check the host, port, credentials, and database name in `database.url`. |
| GitHub private-key error | Set `providers.github.private_key` to a readable PEM file and use the App client ID as `app_id`. |
| Vite fails to start | Install frontend dependencies and check whether another process uses port 5173. |
| Port 8080 is already in use | Stop the existing server, or change both `server.listen` and `server.public_url` to use another port. |

## Check your changes

Run the Rust checks:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

For MySQL integration tests, create a separate database named `deployer_test` and set `DEPLOYER_TEST_DATABASE_URL` to its connection URL before running the tests. Use a dedicated test database; these tests modify its contents.

Run the frontend checks:

```sh
pnpm --dir frontend fmt:check
pnpm --dir frontend test
pnpm --dir frontend build
```

Use the shared components in `frontend/src/components/ui/` for form controls and Reka UI primitives for interactive components.

## Open a pull request

Create a feature branch, keep your changes focused, and open a pull request against `main`. Describe the behavior you changed and the checks you ran. Include screenshots for visible UI changes, and keep credentials and local configuration out of commits.
