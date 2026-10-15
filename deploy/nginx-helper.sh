#!/bin/sh
set -eu

PATH=/usr/sbin:/usr/bin:/sbin:/bin
export PATH

# This file is installed root-owned. Never accept caller-supplied configuration.
[ "$(id -u)" = 0 ] || { echo 'This helper requires root.' >&2; exit 1; }
[ "$#" = 3 ] || exit 2

app_id=$1
domain=$2
mode=$3

case "$app_id" in ''|*[!0-9]*) exit 2 ;; esac
[ "${#app_id}" -le 20 ] || exit 2
[ "${#domain}" -le 253 ] || exit 2
printf '%s\n' "$domain" | grep -Eq '^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$' || exit 2

case "$mode" in
  spa) fallback='/index.html' ;;
  static) fallback='=404' ;;
  *) exit 2 ;;
esac

# Serialize root configuration updates and preserve a working config on failure.
exec 9>/run/lock/deployer-nginx.lock
flock -x 9

config="/etc/nginx/conf.d/deployer-${app_id}.conf"
# App settings are immutable in the MVP. Preserve operator-added TLS settings.
if [ -f "$config" ]; then
  nginx -t
  systemctl reload nginx
  exit 0
fi

backup=$(mktemp /etc/nginx/conf.d/.deployer-backup.XXXXXX)
staged=$(mktemp /etc/nginx/conf.d/.deployer-stage.XXXXXX)
had_config=false

if [ -f "$config" ]; then
  cp "$config" "$backup"
  had_config=true
fi

trap 'rm -f "$backup" "$staged"' EXIT HUP INT TERM

cat > "$staged" <<NGINX
server {
    listen 80;
    server_name ${domain};
    root /var/lib/deployer/apps/${app_id}/current;
    index index.html;

    location / {
        try_files \$uri \$uri/ ${fallback};
    }

    location ~ /\. {
        deny all;
    }

    add_header X-Content-Type-Options nosniff always;
}
NGINX

chmod 644 "$staged"
mv -f "$staged" "$config"

if nginx -t && systemctl reload nginx; then
  exit 0
fi

if [ "$had_config" = true ]; then
  mv -f "$backup" "$config"
else
  rm -f "$config"
fi

nginx -t && systemctl reload nginx
exit 1
