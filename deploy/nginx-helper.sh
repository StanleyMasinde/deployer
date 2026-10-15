#!/bin/sh
set -eu
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export PATH
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
  spa|sync-spa|http-spa|https-spa|challenge-spa) fallback='/index.html' ;;
  static|sync-static|http-static|https-static|challenge-static) fallback='=404' ;;
  remove) fallback='' ;;
  *) exit 2 ;;
esac
ipv6_http=''
if [ -r /proc/net/if_inet6 ] && grep -q . /proc/net/if_inet6; then ipv6_http='    listen [::]:80;'; fi
exec 9>/run/lock/deployer-nginx.lock
flock -x 9
config="/etc/nginx/conf.d/deployer-${app_id}.conf"
if [ -f "$config" ] && { [ "$mode" = spa ] || [ "$mode" = static ]; }; then
  nginx -t
  systemctl reload nginx
  exit 0
fi
backup=$(mktemp /etc/nginx/conf.d/.deployer-backup.XXXXXX)
staged=$(mktemp /etc/nginx/conf.d/.deployer-stage.XXXXXX)
base=$(mktemp /etc/nginx/conf.d/.deployer-base.XXXXXX)
had_config=false
if [ -f "$config" ]; then cp "$config" "$backup"; had_config=true; fi
trap 'rm -f "$backup" "$staged" "$base"' EXIT HUP INT TERM
if [ "$mode" = remove ]; then
  [ "$had_config" = true ] || exit 0
  rm -f "$config"
else
  if [ "$had_config" = true ]; then
    case "$mode" in
      http-*)
        if grep -q '^# Deployer managed HTTPS' "$config"; then
          awk '
            /^# Deployer HTTP redirect/ { exit }
            /^# Deployer managed HTTPS/ { next }
            /# Deployer TLS start/ { tls=1; next }
            /# Deployer TLS end/ { tls=0; next }
            tls { next }
            $1 == "listen" && $2 == "443" { print "    listen 80;"; next }
            $1 == "listen" && $2 == "[::]:443" { print "    listen [::]:80;"; next }
            { print }
          ' "$config" > "$base"
        else
          if grep -Eq '^[[:space:]]*ssl_certificate[[:space:]]' "$config"; then
            echo 'Existing TLS configuration is not managed by Deployer.' >&2; exit 1
          fi
          cp "$config" "$base"
        fi ;;
      *) cp "$config" "$base" ;;
    esac
    challenge=false
    case "$mode" in challenge-*|https-*|http-*) challenge=true ;; esac
    if grep -q '/.well-known/acme-challenge/' "$base"; then challenge=false; fi
    awk -v domain="$domain" -v fallback="$fallback" -v challenge="$challenge" '
      $1 == "server_name" {
        print "    server_name " domain ";"; names++
        if (challenge == "true") {
          print "    location ^~ /.well-known/acme-challenge/ {"
          print "        root /var/lib/deployer/acme-webroot;"
          print "        default_type text/plain;"
          print "        try_files $uri =404;"
          print "    }"
        }
        next
      }
      $1 == "try_files" && $2 == "$uri" && $3 == "$uri/" {
        print "        try_files $uri $uri/ " fallback ";"; routes++; next
      }
      { print }
      END { if (!names || !routes) exit 2 }
    ' "$base" > "$staged"
  else
    cat > "$staged" <<NGINX
server {
    listen 80;
${ipv6_http}
    server_name ${domain};
    root /var/lib/deployer/apps/${app_id}/current;
    index index.html;
    location ^~ /.well-known/acme-challenge/ {
        root /var/lib/deployer/acme-webroot;
        default_type text/plain;
        try_files \$uri =404;
    }
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
  case "$mode" in
    https-*)
      cert="/etc/deployer/tls/${app_id}/${domain}"
      [ -s "$cert/key.pem" ] && [ -s "$cert/fullchain.pem" ] || exit 1
      if ! grep -q '^# Deployer managed HTTPS' "$staged"; then
        # Only convert one HTTP server. Operator-managed TLS requires explicit migration.
        [ "$(grep -Ec '^[[:space:]]*server[[:space:]]*\{' "$staged")" = 1 ] || exit 1
        if grep -Eq '^[[:space:]]*ssl_certificate[[:space:]]' "$staged"; then exit 1; fi
        cp "$staged" "$base"
        printf '# Deployer managed HTTPS\n' > "$staged"
        awk -v cert="$cert" '
          $1 == "listen" && $2 == "80;" { print "    listen 443 ssl;"; next }
          $1 == "listen" && $2 == "[::]:80;" { print "    listen [::]:443 ssl;"; next }
          $1 == "server_name" {
            print
            print "    # Deployer TLS start"
            print "    ssl_certificate " cert "/fullchain.pem;"
            print "    ssl_certificate_key " cert "/key.pem;"
            print "    ssl_protocols TLSv1.2 TLSv1.3;"
            print "    # Deployer TLS end"
            next
          }
          { print }
        ' "$base" >> "$staged"
        cat >> "$staged" <<NGINX
# Deployer HTTP redirect
server {
    listen 80;
${ipv6_http}
    server_name ${domain};
    location ^~ /.well-known/acme-challenge/ {
        root /var/lib/deployer/acme-webroot;
        default_type text/plain;
        try_files \$uri =404;
    }
    location / {
        return 301 https://${domain}\$request_uri;
    }
}
NGINX
      fi ;;
  esac
  chmod 644 "$staged"
  mv -f "$staged" "$config"
fi
if nginx -t && systemctl reload nginx; then exit 0; fi
if [ "$had_config" = true ]; then mv -f "$backup" "$config"; else rm -f "$config"; fi
nginx -t && systemctl reload nginx
exit 1
