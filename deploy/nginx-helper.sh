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
  spa|sync-spa) fallback='/index.html' ;;
  static|sync-static) fallback='=404' ;;
  remove) fallback='' ;;
  *) exit 2 ;;
esac

# Serialize root configuration updates and preserve a working config on failure.
exec 9>/run/lock/deployer-nginx.lock
flock -x 9

config="/etc/nginx/conf.d/deployer-${app_id}.conf"
# Activation preserves operator-added TLS settings. Explicit sync updates managed directives.
if [ -f "$config" ] && { [ "$mode" = spa ] || [ "$mode" = static ]; }; then
  nginx -t
  systemctl reload nginx
  exit 0
fi

if [ "$mode" = remove ]; then
  [ -f "$config" ] || exit 0
  backup=$(mktemp /etc/nginx/conf.d/.deployer-backup.XXXXXX)
  cp "$config" "$backup"
  rm -f "$config"
  if nginx -t && systemctl reload nginx; then
    rm -f "$backup"
    exit 0
  fi
  mv -f "$backup" "$config"
  nginx -t && systemctl reload nginx
  exit 1
fi

backup=$(mktemp /etc/nginx/conf.d/.deployer-backup.XXXXXX)
staged=$(mktemp /etc/nginx/conf.d/.deployer-stage.XXXXXX)
had_config=false

if [ -f "$config" ]; then
  cp "$config" "$backup"
  had_config=true
fi

trap 'rm -f "$backup" "$staged"' EXIT HUP INT TERM

if [ "$had_config" = true ]; then
  # Keep certificates, redirects, and operator directives intact.
  awk -v domain="$domain" -v fallback="$fallback" '
    $1 == "server_name" { print "    server_name " domain ";"; names++; next }
    $1 == "try_files" && $2 == "$uri" && $3 == "$uri/" {
      print "        try_files $uri $uri/ " fallback ";"; routes++; next
    }
    { print }
    END { if (!names || !routes) exit 2 }
  ' "$config" > "$staged"
else
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
fi

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
