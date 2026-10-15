#!/bin/sh
set -eu

repository=StanleyMasinde/deployer
base="https://github.com/$repository/releases"

fail() {
  printf '%s\n' "$*" >&2
  exit 1
}

[ "$(id -u)" = 0 ] || fail 'Run this installer with root privileges: curl -fsSL SCRIPT_URL | sudo sh'
if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != x86_64 ]; then
  fail 'This release supports Linux x86_64 only.'
fi

for tool in curl tar sha256sum systemctl nginx sudo flock openssl install getent useradd groupadd runuser visudo grep sed awk date; do
  command -v "$tool" >/dev/null 2>&1 || fail "Missing prerequiapp: $tool. Install it and run this installer again."
done

# Resolve the release published by Actions, not an ephemeral Actions artifact.
tag=${DEPLOYER_VERSION:-}

if [ -z "$tag" ]; then
  release_url=$(curl --proto '=https' --tlsv1.2 -fsSL -o /dev/null -w '%{url_effective}' "$base/latest")
  tag=${release_url##*/}
fi

printf '%s\n' "$tag" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' || fail 'Invalid release tag. Use vMAJOR.MINOR.PATCH.'
version=${tag#v}
archive="deployer-${tag}-x86_64-unknown-linux-gnu.tar.gz"
work=$(mktemp -d)
trap 'rm -rf "$work"; rm -f /usr/local/libexec/deployer-check' EXIT HUP INT TERM

printf 'Downloading Deployer %s…\n' "$version"
curl --proto '=https' --tlsv1.2 -fsSL "$base/download/$tag/$archive" -o "$work/$archive"
curl --proto '=https' --tlsv1.2 -fsSL "$base/download/$tag/SHA256SUMS" -o "$work/SHA256SUMS"
(cd "$work" && sha256sum -c SHA256SUMS)

mkdir "$work/release"
tar -xzf "$work/$archive" -C "$work/release" --no-same-owner
release="$work/release"

[ "$("$release/bin/deployer" --version)" = "$version" ] || fail 'Binary version does not match the release tag.'
if [ ! -s "$release/assets/$version/index.html" ] || [ ! -s "$release/assets/$version/spa.html" ]; then
  fail 'Release frontend is incomplete.'
fi

# Pin the certificate client and verify it before stopping existing services.
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/acmesh-official/acme.sh/3.1.6/acme.sh \
  -o "$work/acme.sh"
printf '%s  %s\n' c7d68b021cfd6380ea83a82962abde5b484779fee0b97d38681dfa1396bbc8d7 "$work/acme.sh" | sha256sum -c -

if ! getent group deployer >/dev/null; then
  groupadd --system deployer
fi

if ! getent passwd deployer >/dev/null; then
  useradd --system --gid deployer --home-dir /var/lib/deployer --shell /usr/sbin/nologin deployer
fi

install -d -m 750 -o root -g deployer /etc/deployer
install -d -m 750 -o root -g root /etc/sudoers.d
install -d -m 755 -o root -g root /var/lib/deployer /var/lib/deployer/assets /usr/local/libexec /usr/local/bin
install -d -m 755 -o deployer -g deployer /var/lib/deployer/apps
install -d -m 700 -o deployer -g deployer /var/lib/deployer/state

quote_toml() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

prompt() {
  printf '%s: ' "$1" > /dev/tty
  IFS= read -r answer < /dev/tty
  [ -n "$answer" ] || fail 'A value is required.'
}

if [ ! -f /etc/deployer/config.toml ]; then
  if [ -n "${DEPLOYER_CONFIG_FILE:-}" ]; then
    install -m 640 -o root -g deployer "$DEPLOYER_CONFIG_FILE" /etc/deployer/config.toml
  elif [ -r /dev/tty ] && ( : < /dev/tty ) 2>/dev/null; then
    printf '\nConfigure your instance. Create and install your GitHub App first.\n' > /dev/tty

    prompt 'Public HTTPS origin (https://deployer.example.com)'
    public_url=$answer
    prompt 'MySQL connection URL (mysql://USER:PASSWORD@HOST/DATABASE)'
    database_url=$answer
    prompt 'GitHub App client ID'
    app_id=$answer
    prompt 'GitHub installation ID'
    installation_id=$answer
    prompt 'Path to your GitHub App private key PEM'
    private_key=$answer

    # JWT iss is the client ID. The id in
    # /app/installations/{installation_id}/access_tokens is still an integer.
    case "$installation_id" in ''|*[!0-9]*) fail 'Installation ID must be the integer from the installation URL (/installations/123). The client ID belongs in the previous prompt.' ;; esac
    [ -s "$private_key" ] || fail 'Private key file is missing.'

    session_secret=$(openssl rand -hex 32)
    webhook_secret=$(openssl rand -hex 32)
    install -m 640 -o root -g deployer "$private_key" /etc/deployer/github-app.pem

    cat > "$work/config.toml" <<CONFIG
[server]
listen = "127.0.0.1:8080"
public_url = "$(quote_toml "$public_url")"
assets_root = "/var/lib/deployer/assets"
state_root = "/var/lib/deployer/state"
session_secret = "$session_secret"

[database]
url = "$(quote_toml "$database_url")"

[providers]
current = "github"

[providers.github]
app_id = "$(quote_toml "$app_id")"
installation_id = $installation_id
private_key = "/etc/deployer/github-app.pem"
webhook_secret = "$webhook_secret"

[worker]
apps_root = "/var/lib/deployer/apps"
nginx_helper = "/usr/local/libexec/deployer-nginx"
max_artifact_bytes = 52428800
max_extracted_bytes = 209715200
poll_seconds = 2
CONFIG

    install -m 640 -o root -g deployer "$work/config.toml" /etc/deployer/config.toml
  else
    install -m 640 -o root -g deployer "$release/config.toml.example" /etc/deployer/config.toml
    fail 'Complete /etc/deployer/config.toml, then rerun with a configured GitHub App. For unattended installs, set DEPLOYER_CONFIG_FILE.'
  fi
fi

# Repair ownership and modes on upgrades and on unattended installations.
chown root:deployer /etc/deployer/config.toml
chmod 640 /etc/deployer/config.toml

if [ -n "${DEPLOYER_GITHUB_KEY_FILE:-}" ]; then
  install -m 640 -o root -g deployer "$DEPLOYER_GITHUB_KEY_FILE" /etc/deployer/github-app.pem
fi

if [ -f /etc/deployer/github-app.pem ]; then
  chown root:deployer /etc/deployer/github-app.pem
  chmod 640 /etc/deployer/github-app.pem
fi

# Validate as the service user before stopping a running installation.
install -m 755 -o root -g root "$release/bin/deployer" /usr/local/libexec/deployer-check
DEPLOYER_CONFIG=/etc/deployer/config.toml runuser -u deployer -- /usr/local/libexec/deployer-check check-config
rm -f /usr/local/libexec/deployer-check

# Keep configuration and secrets across upgrades. Binaries and frontend are one unit.
systemctl stop deployer-acme.timer deployer-acme.service deployer-worker.service deployer.service 2>/dev/null || true

install -m 755 "$release/bin/deployer" /usr/local/bin/deployer
install -m 755 "$release/bin/deployer-worker" /usr/local/bin/deployer-worker
install -m 755 "$release/bin/deployer-migrate" /usr/local/bin/deployer-migrate
install -m 755 -o root -g root "$release/deploy/nginx-helper.sh" /usr/local/libexec/deployer-nginx
install -d -m 755 -o root -g root /opt/deployer/acme /var/lib/deployer/acme-webroot
install -d -m 700 -o root -g root /var/lib/deployer/acme /etc/deployer/tls
install -m 755 -o root -g root "$work/acme.sh" /opt/deployer/acme/acme.sh
install -m 755 -o root -g root "$release/deploy/acme-helper.sh" /usr/local/libexec/deployer-acme
install -m 644 "$release/deploy/deployer-acme.service" /etc/systemd/system/deployer-acme.service
install -m 644 "$release/deploy/deployer-acme.timer" /etc/systemd/system/deployer-acme.timer
install -m 644 "$release/deploy/deployer.service" /etc/systemd/system/deployer.service
install -m 644 "$release/deploy/deployer-worker.service" /etc/systemd/system/deployer-worker.service
install -m 644 "$release/deploy/control-panel.nginx.conf" /etc/deployer/control-panel.nginx.conf

visudo -cf "$release/deploy/deployer.sudoers"
install -m 440 -o root -g root "$release/deploy/deployer.sudoers" /etc/sudoers.d/deployer

if [ -d "/var/lib/deployer/assets/$version" ]; then
  rm -rf "/var/lib/deployer/assets/$version"
fi

cp -R "$release/assets/$version" "/var/lib/deployer/assets/$version"
chown -R root:root "/var/lib/deployer/assets/$version"
chmod -R a+rX "/var/lib/deployer/assets/$version"
systemctl daemon-reload

# The web server applies embedded migrations on boot. ExecStartPost gates the worker.
systemctl enable --now deployer.service deployer-worker.service deployer-acme.timer
DEPLOYER_CONFIG=/etc/deployer/config.toml runuser -u deployer -- /usr/local/bin/deployer ready

# There is no local release retention; rollback downloads a version from GitHub.
for directory in /var/lib/deployer/assets/*; do
  [ -d "$directory" ] || continue
  [ "$directory" = "/var/lib/deployer/assets/$version" ] || rm -rf "$directory"
done

printf '\nDeployer %s installed. Both systemd services are enabled.\n' "$version"
printf 'Configure the control panel proxy using /etc/deployer/control-panel.nginx.conf and enable HTTPS.\n'
printf 'Open YOUR_PUBLIC_URL/setup to create your root account.\n'
printf 'Read the one-time setup code with: sudo cat /var/lib/deployer/state/setup-code\n'
printf 'Webhook: YOUR_PUBLIC_URL/api/webhooks/github\n'
printf 'Enable HTTPS for each app in the console after pointing DNS at this VPS.\n'
