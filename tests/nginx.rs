#![cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[test]
fn helper_sync_preserves_tls_and_remove_rolls_back_on_failure() {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    let conf = root.path().join("conf");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&conf).unwrap();
    for (name, body) in [
        ("id", "#!/bin/sh\necho 0\n"),
        ("flock", "#!/bin/sh\nexit 0\n"),
        ("nginx", "#!/bin/sh\n[ ! -f \"$FAIL_NGINX\" ]\n"),
        ("systemctl", "#!/bin/sh\nexit 0\n"),
    ] {
        let path = bin.join(name);
        fs::write(&path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let source = include_str!("../deploy/nginx-helper.sh")
        .replace(
            "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
            &format!("PATH={}:/usr/bin:/bin", bin.display()),
        )
        .replace("/etc/nginx/conf.d", conf.to_str().unwrap())
        .replace(
            "/run/lock/deployer-nginx.lock",
            root.path().join("lock").to_str().unwrap(),
        );
    let script = root.path().join("helper.sh");
    fs::write(&script, source).unwrap();
    let failure = root.path().join("fail");
    let run = |mode: &str| {
        Command::new("sh")
            .arg(&script)
            .args(["12", "new.example.com", mode])
            .env("FAIL_NGINX", &failure)
            .output()
            .unwrap()
    };
    let config = conf.join("deployer-12.conf");
    fs::write(&config, "server {\n    listen 443 ssl;\n    server_name old.example.com;\n    ssl_certificate /etc/ssl/operator.pem;\n    location / {\n        try_files $uri $uri/ =404;\n    }\n}\n").unwrap();
    let result = run("sync-spa");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let synced = fs::read_to_string(&config).unwrap();
    assert!(synced.contains("server_name new.example.com;"));
    assert!(synced.contains("try_files $uri $uri/ /index.html;"));
    assert!(synced.contains("ssl_certificate /etc/ssl/operator.pem;"));
    assert!(synced.contains("listen 443 ssl;"));
    fs::write(&failure, "fail").unwrap();
    assert!(!run("sync-static").status.success());
    assert_eq!(fs::read_to_string(&config).unwrap(), synced);
    assert!(!run("remove").status.success());
    assert_eq!(fs::read_to_string(&config).unwrap(), synced);
    fs::remove_file(&failure).unwrap();
    assert!(run("remove").status.success());
    assert!(!config.exists());
    assert!(run("remove").status.success());
    assert!(run("static").status.success());
    assert!(
        fs::read_to_string(&config)
            .unwrap()
            .contains("root /var/lib/deployer/apps/12/current;")
    );
}
