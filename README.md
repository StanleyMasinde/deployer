# Deployer

Deploy static apps and SPAs from GitHub to your VPS. Builds run on GitHub
Actions; native web and worker binaries manage deployments, and Nginx serves
the finished files. No containers are required.

Deployer supports one owner on one Linux x86_64 VPS. You create and own
the GitHub App that connects your repositories.

## Create your GitHub App

Create the App in the GitHub account or organization that owns your repositories:

1. Open **Settings → Developer settings → GitHub Apps → New GitHub App**.
2. Choose a name and use your Deployer control panel URL as the **Homepage URL**.
   Leave user authorization and callback settings unused.
3. Enable webhooks, keep SSL verification enabled, and set **Webhook URL** to
   `https://DEPLOYER_DOMAIN/api/webhooks/github`. Replace `DEPLOYER_DOMAIN` with
   your control panel domain. Set the webhook secret after installation.
4. Set these **Repository permissions**:

   - **Actions** and **Metadata**: read-only, to identify repositories and
     verify builds and download artifacts.
   - **Contents**, **Workflows**, and **Pull requests**: read and write, to
     create the build workflow and open its pull request.
5. Subscribe to **Workflow run** and **Pull request** events, then create the App
   for **Only on this account**. Pull request events update the workflow PR state
   in Deployer when you merge, close, or reopen it. If you already created the App,
   enable **Pull request** under its webhook event subscriptions.
6. Record the **Client ID**. That is the JWT issuer. Generate a private key and
   copy the downloaded PEM file to your VPS. The installer asks for its path.
7. Select **Install App**, install it on the repositories you want to deploy, and
   record the installation ID after `/installations/` in its settings URL. That
   ID is still an integer. It is separate from the client ID.

For GitHub's registration details, see [Registering a GitHub App](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/registering-a-github-app).

## Install

Prepare a systemd-based VPS with MySQL 8.4+, Nginx, curl, sudo, and OpenSSL.
Create a database and your GitHub App, then run the installer:

```sh
curl -fsSL \
  https://raw.githubusercontent.com/StanleyMasinde/deployer/main/install.sh \
  | sudo sh
```

The installer downloads the latest release, prompts for configuration, sets
permissions, and starts both services. Configure DNS and HTTPS, then open
`/setup` on your control panel domain to create your root account.

In your GitHub App settings, set **Webhook secret** to the
`[providers.github].webhook_secret` value from `/etc/deployer/config.toml`.
The installer generates it. GitHub and Deployer must use the same value.

## Deploy a app

Add a repository, branch, domain, build commands, and output directory in the
console. Enable SPA routing if your app needs an `index.html` fallback.

Merge the generated workflow PR. Pushes to your selected branch build the app
on GitHub Actions. A completed-build webhook queues the worker to verify and
download the artifact, then publish the files through Nginx.

## Enable HTTPS for an app

The installer installs a pinned, checksum-verified acme.sh client and enables
`deployer-acme.timer` for daily renewal checks. Deployer uses Let's Encrypt
with HTTP validation and keeps ownership of the app's Nginx configuration.

1. Point the app domain's DNS records at your VPS. If you publish an AAAA
   record, make sure it reaches this VPS over IPv6 too.
2. Allow inbound TCP ports 80 and 443. Keep port 80 accessible for renewals.
3. Open the app's **Details**, then select **Enable HTTPS**.
4. Wait for **HTTPS active**. The details view updates automatically and shows
   the certificate expiry date. You can enable HTTPS before the first deployment.

Deployer redirects HTTP requests to HTTPS after issuance, while keeping
`/.well-known/acme-challenge/` reachable for validation. Certificates and keys
live under `/etc/deployer/tls/APP_ID/`; ACME account and renewal state live
under `/var/lib/deployer/acme/`. Replace `APP_ID` with the application's ID.
Only the root-owned helper handles private keys.

If issuance fails, check DNS and port 80, then select **Retry certificate**.
Inspect worker and renewal logs on the VPS:

```sh
sudo journalctl -u deployer-worker.service -n 100
sudo journalctl -u deployer-acme.service -n 100
sudo systemctl status deployer-acme.timer
```

To run a renewal check immediately, start the renewal service:

```sh
sudo systemctl start deployer-acme.service
```

The client renews certificates when due. After renewal, its saved hook validates
and reloads Nginx. The worker refreshes expiry and renewal-error information
about once a minute while idle; a busy worker can delay the update.

Changing an HTTPS-enabled app's domain queues a certificate request for the new
hostname. HTTP remains available if issuance fails. Deleting an app removes
its Nginx config, certificates, and per-app renewal state through the worker.

Existing TLS configurations added outside Deployer aren't adopted automatically.
Keep managing those certificates separately, or migrate the app to an HTTP
configuration before enabling managed HTTPS. The control panel's own HTTPS
configuration remains separate from app certificate management.

For upgrades, rerun the installer so the worker, helpers, acme.sh client, and
renewal units are installed together. Local macOS development can exercise the
console, but live certificate issuance requires the Linux VPS setup and public
DNS. The helper tests simulate the ACME client without requesting public certificates.

## Contributing

See [Contributing](Contributing.md) to run Deployer locally and check your changes.

## Recover your password

Configure SMTP and keep the worker running to deliver password-reset email. Use
**Forgot password?** on the sign-in page. Deployer returns the same response for
recognized and unrecognized addresses. It sends at most one reset email per
account per minute.

Reset links expire after 30 minutes and work once. Production links require the
configured HTTPS control-panel URL; localhost HTTP is allowed in debug builds.
The token stays in the URL fragment, so browsers do not send it in HTTP request
URLs. A successful reset invalidates existing sessions, queues a notification
email, and requires you to sign in again. New passwords require at least 15
characters and at most 1024 UTF-8 bytes.

This release binds sessions to the account's password hash. Existing sessions
issued by earlier releases must sign in again after upgrading.
