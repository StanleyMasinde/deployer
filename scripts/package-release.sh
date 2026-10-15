#!/bin/sh
set -eu

version=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
tag=${GITHUB_REF_NAME:-v$version}

[ "$tag" = "v$version" ] || { echo 'Git tag must equal v<Cargo version>.' >&2; exit 1; }

build_root=${BUILD_ROOT:-target/x86_64-unknown-linux-gnu/release}
staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT HUP INT TERM

mkdir -p "$staging/bin" "$staging/assets/$version" dist
install -m 755 "$build_root/deployer" "$staging/bin/deployer"
install -m 755 "$build_root/worker" "$staging/bin/deployer-worker"
install -m 755 "$build_root/migrate" "$staging/bin/deployer-migrate"
cp -R frontend/dist/. "$staging/assets/$version/"
cp -R deploy "$staging/deploy"
cp config.toml.example "$staging/"

archive="deployer-${tag}-x86_64-unknown-linux-gnu.tar.gz"
tar -czf "dist/$archive" -C "$staging" .

(cd dist && sha256sum "$archive" > SHA256SUMS)
