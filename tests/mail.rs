use deployer::{
    config::{Mail, Smtp, SmtpSecurity},
    mail::{EmailSender, SmtpSender, SystemEmail, validate_address},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};

fn config(port: u16) -> Mail {
    Mail {
        from_email: "deployer@example.com".into(),
        from_name: "Deployer".into(),
        smtp: Smtp {
            host: "127.0.0.1".into(),
            port,
            security: SmtpSecurity::None,
            username: None,
            password: None,
        },
    }
}

#[test]
fn email_validation_and_template_escaping() {
    for address in ["owner@example.com", "owner+deployer@example.com"] {
        assert!(validate_address(address).is_ok());
    }

    for address in [
        "",
        "invalid",
        "Name <owner@example.com>",
        "owner@example.com\r\nBcc: other@example.com",
    ] {
        assert!(validate_address(address).is_err());
    }

    let mut email = SystemEmail::Welcome {
        name: "<owner>",
        link: "https://deployer.example.com",
    }
    .render("owner@example.com")
    .unwrap();

    assert!(email.text.contains("<owner>"));
    assert!(email.html.contains("&#60;owner&#62;"));
    assert!(email.html.contains("https://deployer.example.com/console"));
    email.subject = "Hello\r\nBcc: other@example.com".into();
    assert!(email.validate().is_err());
}

#[test]
fn smtp_configuration_requires_paired_credentials_and_secure_remote_transport() {
    let mut mail = config(1025);
    assert!(mail.validate().is_ok());
    mail.smtp.username = Some("user".into());
    assert!(mail.validate().is_err());
    mail.smtp.password = Some("password".into());
    assert!(mail.validate().is_ok());
    mail.smtp.host = "smtp.example.com".into();
    assert!(mail.validate().is_err());
    mail.smtp.security = SmtpSecurity::Starttls;
    assert!(mail.validate().is_ok());
    mail.smtp.security = SmtpSecurity::Tls;
    assert!(mail.validate().is_ok());
}

async fn smtp_server(recipient_response: &'static str) -> (u16, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut stream = BufReader::new(stream);
        stream
            .get_mut()
            .write_all(b"220 localhost test SMTP\r\n")
            .await
            .unwrap();
        let mut message = String::new();
        let mut data = false;

        loop {
            let mut line = String::new();

            if stream.read_line(&mut line).await.unwrap() == 0 {
                break;
            }

            let reply = if data {
                if line == ".\r\n" {
                    data = false;
                    "250 accepted\r\n"
                } else {
                    message.push_str(&line);
                    continue;
                }
            } else if line.starts_with("EHLO") {
                "250 localhost\r\n"
            } else if line.starts_with("RCPT TO:") {
                recipient_response
            } else if line.starts_with("DATA") {
                data = true;
                "354 send message\r\n"
            } else if line.starts_with("QUIT") {
                stream.get_mut().write_all(b"221 bye\r\n").await.unwrap();
                break;
            } else {
                "250 ok\r\n"
            };

            stream.get_mut().write_all(reply.as_bytes()).await.unwrap();
        }

        message
    });

    (port, server)
}

#[tokio::test]
async fn smtp_delivery_emits_multipart_and_classifies_rejections() {
    for (reply, expected) in [
        ("250 recipient ok\r\n", None),
        ("451 try later\r\n", Some((true, "smtp_unavailable"))),
        ("550 rejected\r\n", Some((false, "smtp_rejected"))),
    ] {
        let (port, server) = smtp_server(reply).await;
        let sender = SmtpSender::new(&config(port)).unwrap();
        let email = SystemEmail::Welcome {
            name: "owner",
            link: "https://deployer.example.com",
        }
        .render("owner@example.com")
        .unwrap();
        let result = sender.deliver(42, &email).await;

        match expected {
            None => assert!(result.is_ok()),
            Some((retryable, code)) => {
                let failure = result.unwrap_err();
                assert_eq!(failure.retryable, retryable);
                assert_eq!(failure.code, code);
            }
        }

        drop(sender);
        let message = tokio::time::timeout(std::time::Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();

        if expected.is_none() {
            assert!(message.contains("multipart/alternative"));
            assert!(message.contains("text/plain"));
            assert!(message.contains("text/html"));
            assert!(message.contains("<deployer-job-42@example.com>"));
            assert!(message.contains("To: owner@example.com"));
        }
    }
}
