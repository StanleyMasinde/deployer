#![cfg(unix)]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Sandbox {
    root: tempfile::TempDir,
    helper: PathBuf,
}
impl Sandbox {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        for dir in ["bin", "conf", "client", "state", "tls"] {
            fs::create_dir_all(root.path().join(dir)).unwrap();
        }
        let write = |path: &Path, text: &str| {
            fs::write(path, text).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        };
        for (name, body) in [
            ("id", "#!/bin/sh\necho 0\n"),
            ("flock", "#!/bin/sh\nexit 0\n"),
            ("nginx", "#!/bin/sh\n[ ! -f \"$MOCK_ROOT/nginx-fail\" ]\n"),
            ("systemctl", "#!/bin/sh\nexit 0\n"),
            ("date", "#!/bin/sh\nprintf '%s\\n' 2000000000\n"),
            (
                "openssl",
                r#"#!/bin/sh
set -eu
cert=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    -in) cert=$2; shift ;;
    -checkhost) [ "$(cat "$cert")" = "$2" ] || exit 1; shift ;;
    -checkend) [ ! -f "$MOCK_ROOT/expired" ] || exit 1; shift ;;
    -enddate) printf 'notAfter=May 18 03:33:20 2033 GMT\n' ;;
  esac
  shift
done
"#,
            ),
        ] {
            write(&root.path().join("bin").join(name), body);
        }
        write(
            &root.path().join("client/acme.sh"),
            r#"#!/bin/sh
set -eu
action=''; domain=''; key=''; chain=''
printf '%s\n' "$*" >> "$MOCK_ROOT/client-calls"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --issue|--install-cert|--cron|--remove) action=$1 ;;
    -d) domain=$2; shift ;;
    --key-file) key=$2; shift ;;
    --fullchain-file) chain=$2; shift ;;
  esac
  shift
done
case "$action" in
  --issue) [ ! -f "$MOCK_ROOT/issue-fail" ] || exit 1; [ ! -f "$MOCK_ROOT/skip" ] || exit 2 ;;
  --install-cert) printf 'private-key\n' > "$key"; printf '%s\n' "$domain" > "$chain" ;;
  --cron) [ ! -f "$MOCK_ROOT/renew-fail" ] || exit 1 ;;
esac
"#,
        );
        let rewrite = |source: &str| {
            source
                .replace(
                    "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
                    &format!("PATH={}:/usr/bin:/bin", root.path().join("bin").display()),
                )
                .replace(
                    "/opt/deployer/acme",
                    root.path().join("client").to_str().unwrap(),
                )
                .replace(
                    "/var/lib/deployer/acme",
                    root.path().join("state").to_str().unwrap(),
                )
                .replace(
                    "/etc/deployer/tls",
                    root.path().join("tls").to_str().unwrap(),
                )
                .replace(
                    "/etc/nginx/conf.d",
                    root.path().join("conf").to_str().unwrap(),
                )
                .replace(
                    "/usr/local/libexec/deployer-nginx",
                    root.path().join("nginx-helper").to_str().unwrap(),
                )
                .replace(
                    "/usr/local/libexec/deployer-acme",
                    root.path().join("acme-helper").to_str().unwrap(),
                )
                .replace(
                    "/run/lock/deployer-nginx.lock",
                    root.path().join("nginx-lock").to_str().unwrap(),
                )
                .replace(
                    "/run/lock/deployer-acme.lock",
                    root.path().join("acme-lock").to_str().unwrap(),
                )
        };
        let helper = root.path().join("acme-helper");
        write(&helper, &rewrite(include_str!("../deploy/acme-helper.sh")));
        write(
            &root.path().join("nginx-helper"),
            &rewrite(include_str!("../deploy/nginx-helper.sh")),
        );
        Self { root, helper }
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new("sh")
            .arg(&self.helper)
            .args(args)
            .env("MOCK_ROOT", self.root.path())
            .output()
            .unwrap()
    }
    fn config(&self) -> String {
        fs::read_to_string(self.root.path().join("conf/deployer-12.conf")).unwrap()
    }
}

#[test]
fn issues_before_deployment_renews_changes_domain_and_cleans_up() {
    let fixture = Sandbox::new();
    let out = fixture.run(&["issue", "12", "site.example.com", "spa"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let status: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(status["expiresAt"], 2_000_000_000_i64);
    let config = fixture.config();
    assert!(config.contains("listen 443 ssl;"));
    assert!(config.contains("return 301 https://site.example.com$request_uri;"));
    assert!(config.contains("location ^~ /.well-known/acme-challenge/"));
    assert!(config.contains("try_files $uri $uri/ /index.html;"));
    let key = fixture.root.path().join("tls/12/site.example.com/key.pem");
    assert_eq!(
        fs::metadata(key).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::write(fixture.root.path().join("renew-fail"), "").unwrap();
    assert!(!fixture.run(&["renew"]).status.success());
    let out = fixture.run(&["status", "12", "site.example.com", "spa"]);
    let status: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(status["renewalFailed"], true);
    fs::remove_file(fixture.root.path().join("renew-fail")).unwrap();
    assert!(fixture.run(&["renew"]).status.success());
    fs::write(fixture.root.path().join("skip"), "").unwrap();
    assert!(
        fixture
            .run(&["issue", "12", "site.example.com", "static"])
            .status
            .success()
    );
    fs::remove_file(fixture.root.path().join("skip")).unwrap();
    assert!(
        fixture
            .run(&["issue", "12", "new.example.com", "static"])
            .status
            .success()
    );
    let config = fixture.config();
    assert!(config.contains("return 301 https://new.example.com$request_uri;"));
    assert!(!config.contains("site.example.com"));
    let calls = fs::read_to_string(fixture.root.path().join("client-calls")).unwrap();
    assert!(calls.contains("--server letsencrypt --webroot"));
    assert!(calls.contains("--remove --ecc -d site.example.com"));
    assert!(
        fixture
            .run(&["cleanup", "12", "new.example.com", "static"])
            .status
            .success()
    );
    assert!(!fixture.root.path().join("tls/12").exists());
    assert!(!fixture.root.path().join("state/certs/12").exists());
}

#[test]
fn failure_keeps_http_available_and_rejects_bad_input_or_unmanaged_tls() {
    let fixture = Sandbox::new();
    assert!(
        !fixture
            .run(&["issue", "../12", "site.example.com", "static"])
            .status
            .success()
    );
    assert!(
        !fixture
            .run(&["issue", "12", "site.example.com;touch", "static"])
            .status
            .success()
    );
    fs::write(fixture.root.path().join("issue-fail"), "").unwrap();
    assert!(
        !fixture
            .run(&["issue", "12", "site.example.com", "static"])
            .status
            .success()
    );
    let config = fixture.config();
    assert!(config.contains("listen 80;"));
    assert!(!config.contains("listen 443"));
    assert!(!fixture.root.path().join("tls/12/domain").exists());
    fs::remove_file(fixture.root.path().join("issue-fail")).unwrap();
    fs::write(fixture.root.path().join("expired"), "").unwrap();
    assert!(
        !fixture
            .run(&["issue", "12", "site.example.com", "static"])
            .status
            .success()
    );
    assert!(!fixture.config().contains("listen 443"));
    fs::write(
        fixture.root.path().join("conf/deployer-12.conf"),
        "server {\n ssl_certificate /operator/cert.pem;\n}\n",
    )
    .unwrap();
    assert!(
        !fixture
            .run(&["issue", "12", "site.example.com", "static"])
            .status
            .success()
    );
    assert!(fixture.config().contains("/operator/cert.pem"));
}
