#!/bin/sh
set -eu
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export PATH
# Updates come through the checksum-verified Deployer installer.
ACME_PACKAGED=1
export ACME_PACKAGED
[ "$(id -u)" = 0 ] || { echo 'This helper requires root.' >&2; exit 1; }
umask 077
client=/opt/deployer/acme/acme.sh
state=/var/lib/deployer/acme
certificates=/etc/deployer/tls
nginx_helper=/usr/local/libexec/deployer-nginx

reload_nginx() {
  exec 8>/run/lock/deployer-nginx.lock
  flock -x 8
  nginx -t && systemctl reload nginx
}

# Renewal hooks take only the Nginx lock, never the ACME lock already held by renewal.
if [ "$#" = 1 ] && [ "$1" = reload ]; then reload_nginx; exit; fi
acme() { "$client" --home /opt/deployer/acme --config-home "$state" --cert-home "$state/certs/$app_id" "$@"; }

if [ "$#" = 1 ] && [ "$1" = renew ]; then
  exec 9>/run/lock/deployer-acme.lock
  flock -x 9
  result=0
  for manifest in "$certificates"/*/domain; do
    [ -f "$manifest" ] || continue
    app_id=${manifest%/domain}; app_id=${app_id##*/}
    case "$app_id" in ''|*[!0-9]*) continue ;; esac
    if acme --cron >&2; then
      rm -f "$certificates/$app_id/renewal-error"
    else
      printf 'renewal_failed\n' > "$certificates/$app_id/renewal-error"
      result=1
    fi
  done
  exit "$result"
fi

[ "$#" = 4 ] || exit 2
action=$1
app_id=$2
domain=$3
routing=$4
case "$app_id" in ''|*[!0-9]*) exit 2 ;; esac
[ "${#app_id}" -le 20 ] || exit 2
[ "${#domain}" -le 253 ] || exit 2
printf '%s\n' "$domain" | grep -Eq '^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$' || exit 2
case "$routing" in spa|static) ;; *) exit 2 ;; esac
case "$action" in issue|status|cleanup) ;; *) exit 2 ;; esac
cert="$certificates/$app_id/$domain"


certificate_status() {
  openssl x509 -in "$cert/fullchain.pem" -checkhost "$domain" -noout >&2
  expiry=$(openssl x509 -in "$cert/fullchain.pem" -enddate -noout)
  expiry=$(date -u -d "${expiry#notAfter=}" +%s)
  failed=false
  if [ -f "$certificates/$app_id/renewal-error" ]; then failed=true; fi
  printf '{"expiresAt":%s,"renewalFailed":%s}\n' "$expiry" "$failed"
}
if [ "$action" = status ]; then certificate_status; exit; fi
exec 9>/run/lock/deployer-acme.lock
flock -x 9
if [ "$action" = cleanup ]; then
  rm -rf "${certificates:?}/$app_id" "${state:?}/certs/$app_id"
  exit 0
fi


[ -x "$client" ] || { echo 'acme.sh is not installed. Rerun the Deployer installer.' >&2; exit 4; }
config="/etc/nginx/conf.d/deployer-${app_id}.conf"
if [ -f "$config" ] && grep -Eq '^[[:space:]]*ssl_certificate[[:space:]]' "$config" && ! grep -q '^# Deployer managed HTTPS' "$config"; then
  echo 'Existing TLS configuration is not managed by Deployer. Migrate it before enabling managed HTTPS.' >&2
  exit 3
fi
old_domain=''
if [ -f "$certificates/$app_id/domain" ]; then old_domain=$(cat "$certificates/$app_id/domain"); fi
# A changed domain must answer HTTP validation without serving the old hostname's certificate.
if [ "$old_domain" != "$domain" ]; then
  "$nginx_helper" "$app_id" "$domain" "http-$routing" >&2
else
  "$nginx_helper" "$app_id" "$domain" "challenge-$routing" >&2
fi
install -d -m 700 "$state" "$state/certs/$app_id" "$certificates/$app_id" "$cert"
install -d -m 755 /var/lib/deployer/acme-webroot /var/lib/deployer/acme-webroot/.well-known /var/lib/deployer/acme-webroot/.well-known/acme-challenge
# Use public challenge permissions; account and private-key directories remain root-only.
result=0
(umask 022; acme --issue --server letsencrypt --webroot /var/lib/deployer/acme-webroot -d "$domain" --keylength ec-256) >&2 || result=$?
# acme.sh returns 2 when an existing certificate is not due for renewal.
[ "$result" = 0 ] || [ "$result" = 2 ] || exit "$result"
acme --install-cert --ecc -d "$domain" --key-file "$cert/key.pem" --fullchain-file "$cert/fullchain.pem" --reloadcmd '/usr/local/libexec/deployer-acme reload' >&2
chmod 600 "$cert/key.pem" "$cert/fullchain.pem"
openssl x509 -in "$cert/fullchain.pem" -checkhost "$domain" -noout >&2
openssl x509 -in "$cert/fullchain.pem" -checkend 0 -noout >&2
"$nginx_helper" "$app_id" "$domain" "https-$routing" >&2
printf '%s\n' "$domain" > "$certificates/$app_id/domain"
rm -f "$certificates/$app_id/renewal-error"
if [ -n "$old_domain" ] && [ "$old_domain" != "$domain" ]; then
  acme --remove --ecc -d "$old_domain" >&2 || true
fi
certificate_status
