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
5. Subscribe to **Workflow run** events and create the App for **Only on this
   account**.
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
