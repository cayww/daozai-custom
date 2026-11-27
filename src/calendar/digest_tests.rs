use super::{fetch_month, Config, Provider};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::Duration;

const USER: &str = "calendar-user";
const PASSWORD: &str = "calendar-password";
const REALM: &str = "calendar, suite";
const OPAQUE: &str = "test, opaque";

struct Request {
    method: String,
    target: String,
    authorization: String,
    depth: String,
    body: String,
}

impl Request {
    fn read(socket: &mut TcpStream) -> Self {
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut reader = BufReader::new(socket);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let mut request_line = line.split_ascii_whitespace();
        let method = request_line.next().unwrap().to_owned();
        let target = request_line.next().unwrap().to_owned();
        let mut authorization = String::new();
        let mut depth = String::new();
        let mut length = 0;
        loop {
            line.clear();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            if line == "\r\n" {
                break;
            }
            let (name, value) = line.trim_end().split_once(':').unwrap();
            match name.to_ascii_lowercase().as_str() {
                "authorization" => authorization = value.trim().to_owned(),
                "depth" => depth = value.trim().to_owned(),
                "content-length" => length = value.trim().parse().unwrap(),
                _ => {}
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        Self {
            method,
            target,
            authorization,
            depth,
            body: String::from_utf8(body).unwrap(),
        }
    }

    fn has_digest(&self) -> bool {
        self.authorization.starts_with("Digest ")
    }
}

struct Reply(u16, String, &'static str);

impl Reply {
    fn send(self, socket: &mut TcpStream) {
        let Self(code, headers, body) = self;
        write!(socket, "HTTP/1.1 {code} Test\r\n{headers}Content-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    }
}

fn with_server<T>(
    mut respond: impl FnMut(Request) -> Reply + Send + 'static,
    run: impl FnOnce(Config) -> T,
) -> (T, usize) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let (stop, stopped) = mpsc::channel::<()>();
    let worker = thread::spawn(move || {
        let mut count = 0;
        while stopped.try_recv() == Err(TryRecvError::Empty) {
            match listener.accept() {
                Ok((mut socket, _)) => {
                    count += 1;
                    assert!(count <= 20, "CalDAV 请求超过测试预期");
                    respond(Request::read(&mut socket)).send(&mut socket);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2))
                }
                Err(error) => panic!("CalDAV 测试服务监听失败：{error}"),
            }
        }
        count
    });
    let result = run(Config {
        provider: Provider::Custom,
        username: USER.into(),
        href: None,
        server: Some(base),
    });
    drop(stop);
    (result, worker.join().unwrap())
}

fn challenge(algorithm: &str, nonce: &str, stale: bool) -> Reply {
    let stale = if stale { ", stale=true" } else { "" };
    Reply(401, format!("WWW-Authenticate: Digest realm=\"{REALM}\", nonce=\"{nonce}\", opaque=\"{OPAQUE}\", algorithm={algorithm}, qop=\"auth\"{stale}\r\n"), "")
}

fn field<'a>(authorization: &'a str, name: &str) -> &'a str {
    let prefix = format!("{name}=");
    let start = authorization
        .match_indices(&prefix)
        .find(|(index, _)| {
            *index > 0 && matches!(authorization.as_bytes()[*index - 1], b' ' | b',')
        })
        .unwrap_or_else(|| panic!("Digest 缺少 {name}"))
        .0;
    let value = &authorization[start + prefix.len()..];
    if let Some(quoted) = value.strip_prefix('"') {
        quoted.split_once('"').unwrap().0
    } else {
        value.split(',').next().unwrap().trim()
    }
}

fn verify_digest(request: &Request, nonce: &str, algorithm: &str) {
    let hash = |input: &str| {
        let digest = ring::digest::digest(&ring::digest::SHA256, input.as_bytes());
        #[cfg(windows)]
        let mut md5 = [0; 16];
        let bytes = match algorithm {
            #[cfg(windows)]
            "MD5" => {
                use windows::Win32::Security::Cryptography::{BCryptHash, BCRYPT_MD5_ALG_HANDLE};
                assert_eq!(
                    unsafe { BCryptHash(BCRYPT_MD5_ALG_HANDLE, None, input.as_bytes(), &mut md5) }
                        .0,
                    0
                );
                md5.as_slice()
            }
            "SHA-256" => digest.as_ref(),
            _ => unreachable!(),
        };
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    assert!(request.has_digest());
    let auth = &request.authorization;
    assert_eq!(field(auth, "username"), USER);
    assert_eq!(field(auth, "realm"), REALM);
    assert_eq!(field(auth, "nonce"), nonce);
    assert_eq!(field(auth, "opaque"), OPAQUE);
    assert_eq!(field(auth, "uri"), request.target);
    assert_eq!(field(auth, "algorithm"), algorithm);
    assert_eq!(field(auth, "qop"), "auth");
    let count = field(auth, "nc");
    let client_nonce = field(auth, "cnonce");
    assert_eq!(count.len(), 8);
    assert!(u32::from_str_radix(count, 16).unwrap() > 0);
    assert!(!client_nonce.is_empty());
    let account = hash(&format!("{USER}:{REALM}:{PASSWORD}"));
    let target = hash(&format!("{}:{}", request.method, request.target));
    assert_eq!(
        field(auth, "response"),
        hash(&format!(
            "{account}:{nonce}:{count}:{client_nonce}:auth:{target}"
        ))
    );
}

#[test]
fn basic_calendar_keeps_directory_fallback_and_reports_resource_permissions() {
    const EMPTY: &str = r#"<multistatus xmlns="DAV:"/>"#;
    const DISCOVERY: &str = r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:href>/dav/</d:href><d:resourcetype><c:calendar/></d:resourcetype></d:response></d:multistatus>"#;
    const DENIED: &str = r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/dav/hidden.ics</d:href><d:status>HTTP/1.1 401 Unauthorized</d:status></d:response></d:multistatus>"#;
    for (initial_status, denied) in [(207, false), (207, true), (400, false), (401, false), (403, false)] {
        let (result, count) = with_server(
            move |request| {
                assert_eq!(request.authorization, super::basic_auth(USER, PASSWORD));
                match (request.method.as_str(), request.target.as_str()) {
                    ("PROPFIND", "/dav") => Reply(
                        initial_status,
                        if initial_status == 401 { "WWW-Authenticate: Basic realm=\"entry\"\r\n".into() } else { String::new() },
                        EMPTY,
                    ),
                    ("PROPFIND", "/dav/") => Reply(207, String::new(), DISCOVERY),
                    ("REPORT", "/dav/") => {
                        Reply(207, String::new(), if denied { DENIED } else { EMPTY })
                    }
                    _ => panic!("意外的 Basic 日历请求"),
                }
            },
            |mut config| {
                config.server.as_mut().unwrap().push_str("dav");
                fetch_month(
                    &config,
                    PASSWORD,
                    chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
                )
            },
        );
        if denied {
            let error = result.err().unwrap();
            assert_eq!(error.kind, "resource_permission");
            assert!(error.message.contains("读取权限"));
        } else {
            assert!(result.unwrap().events.is_empty());
        }
        assert_eq!(count, 3);
    }
}

#[cfg(windows)]
#[test]
fn wps_digest_discovers_calendar_and_reads_events() {
    const PRINCIPAL: &str = r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/caldav/principals/</d:href><d:propstat><d:prop><d:current-user-principal><d:href>/caldav/principals/account/</d:href></d:current-user-principal></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#;
    const HOME: &str = r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:href>/caldav/principals/account/</d:href><d:propstat><d:prop><c:calendar-home-set><d:href>/caldav/calendar/account/</d:href></c:calendar-home-set></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#;
    const CALENDAR: &str = r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:href>/caldav/calendar/account/r/work/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/><c:calendar/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#;
    const EVENT: &str = "<d:multistatus xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\"><d:response><d:href>/caldav/calendar/account/r/work/1.ics</d:href><c:calendar-data>BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:digest-event\nDTSTART:20261112T040000Z\nSUMMARY:工作日程\nEND:VEVENT\nEND:VCALENDAR</c:calendar-data></d:response></d:multistatus>";
    let mut signed_paths = Vec::new();
    let (result, _) = with_server(
        move |request| {
            if request.target == "/.well-known/caldav" {
                assert_eq!(signed_paths, ["/"]);
                return Reply(301, "Location: /caldav/principals/\r\n".into(), "");
            }
            if !request.has_digest() {
                return challenge("MD5", "wps-nonce", false);
            }
            verify_digest(&request, "wps-nonce", "MD5");
            signed_paths.push(request.target.clone());
            let (code, body) = match (request.method.as_str(), request.target.as_str()) {
                ("PROPFIND", "/") => return challenge("MD5", "root-not-a-collection", false),
                ("PROPFIND", "/caldav/principals/") => (207, PRINCIPAL),
                ("PROPFIND", "/caldav/principals/account/") => (207, HOME),
                ("PROPFIND", "/caldav/calendar/account/") => (207, CALENDAR),
                ("REPORT", "/caldav/calendar/account/r/work/") => (207, EVENT),
                _ => panic!("意外的 CalDAV 请求：{} {}", request.method, request.target),
            };
            Reply(code, String::new(), body)
        },
        |cfg| fetch_month(&cfg, PASSWORD, "2026-11-01".parse().unwrap()),
    );
    let result = result.unwrap();
    assert!(result.href.ends_with("/caldav/calendar/account/r/work/"));
    assert_eq!(
        result
            .events
            .iter()
            .map(|event| event.uid.as_str())
            .collect::<Vec<_>>(),
        ["digest-event"]
    );
}

#[test]
fn digest_retries_expired_nonce_and_signs_the_redirected_target() {
    const START: &str = "/calendar/start/?uid=1";
    const TARGET: &str = "/calendar/%E6%97%A5%E5%8E%86/?uid=2%2B3";
    const DISCOVERY: &str = r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:href>/calendar/start/?uid=1</d:href><d:resourcetype><c:calendar/></d:resourcetype></d:response></d:multistatus>"#;
    let mut attempt = 0;
    let mut report_body = String::new();
    let (result, count) = with_server(
        move |request| {
            if request.method == "PROPFIND" {
                assert_eq!(request.target, "/discover?account=a%2Bb");
                return Reply(207, String::new(), DISCOVERY);
            }
            assert_eq!(request.method, "REPORT");
            assert_eq!(request.depth, "1");
            attempt += 1;
            if attempt == 1 {
                assert_eq!(request.target, START);
                assert!(request.body.contains("calendar-query"));
                report_body = request.body;
                return Reply(307, format!("Location: {TARGET}\r\n"), "");
            }
            assert_eq!(request.target, TARGET);
            if attempt <= 4 {
                assert_eq!(request.body, report_body);
            } else {
                assert!(request.body.contains("calendar-multiget"));
                assert!(request
                    .body
                    .contains("/calendar/%E6%97%A5%E5%8E%86/one.ics"));
            }
            match attempt {
                2 => {
                    let mut reply = challenge("SHA-256", "old-nonce", false);
                    reply
                        .1
                        .insert_str(0, "WWW-Authenticate: Basic realm=\"legacy\"\r\n");
                    reply
                }
                3 => {
                    verify_digest(&request, "old-nonce", "SHA-256");
                    challenge("SHA-256", "new-nonce", true)
                }
                4 => {
                    verify_digest(&request, "new-nonce", "SHA-256");
                    Reply(
                        207,
                        String::new(),
                        r#"<multistatus xmlns="DAV:"><response><href>one.ics</href></response></multistatus>"#,
                    )
                }
                5 => {
                    assert!(request.authorization.is_empty());
                    challenge("SHA-256", "multiget-nonce", false)
                }
                6 => {
                    verify_digest(&request, "multiget-nonce", "SHA-256");
                    Reply(207, String::new(), "<d:multistatus xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\"><d:response><d:href>one.ics</d:href><c:calendar-data>BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:redirected-event\nDTSTART:20261112T040000Z\nSUMMARY:跳转后的日程\nEND:VEVENT\nEND:VCALENDAR</c:calendar-data></d:response></d:multistatus>")
                }
                _ => panic!("Digest 重试不应继续"),
            }
        },
        |mut cfg| {
            cfg.server
                .as_mut()
                .unwrap()
                .push_str("discover?account=a%2Bb");
            fetch_month(&cfg, PASSWORD, "2026-11-01".parse().unwrap())
        },
    );
    assert_eq!(result.unwrap().events[0].uid, "redirected-event");
    assert_eq!(count, 7);
}

#[test]
fn digest_rejection_does_not_keep_retrying_or_probe_other_paths() {
    let (result, count) = with_server(
        |request| {
            assert_eq!(request.target, "/calendar/");
            if request.has_digest() {
                verify_digest(&request, "rejected-nonce", "SHA-256");
            }
            challenge("SHA-256", "rejected-nonce", false)
        },
        |mut cfg| {
            cfg.server.as_mut().unwrap().push_str("calendar/");
            fetch_month(&cfg, PASSWORD, "2026-11-01".parse().unwrap())
        },
    );
    assert_eq!(result.err().unwrap().status, Some(401));
    assert_eq!(count, 2);
}

#[test]
fn digest_redirect_does_not_contact_a_different_origin() {
    let other = TcpListener::bind("127.0.0.1:0").unwrap();
    other.set_nonblocking(true).unwrap();
    let redirect = format!(
        "Location: http://{}/calendar/\r\n",
        other.local_addr().unwrap()
    );
    let (result, count) = with_server(
        move |request| {
            if request.has_digest() {
                verify_digest(&request, "redirect-nonce", "SHA-256");
                Reply(307, redirect.clone(), "")
            } else {
                challenge("SHA-256", "redirect-nonce", false)
            }
        },
        |cfg| fetch_month(&cfg, PASSWORD, "2026-11-01".parse().unwrap()),
    );
    assert_eq!(count, 2);
    assert!(result.err().unwrap().message.contains("不可信地址"));
    assert_eq!(
        other.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
