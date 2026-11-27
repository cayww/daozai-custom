use digest_auth::{Algorithm, AlgorithmType, AuthContext, Charset, Qop, WwwAuthenticateHeader};
use std::collections::HashMap;
use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChallengeError {
    Malformed,
    Unsupported,
}

pub(super) struct Challenge(WwwAuthenticateHeader);

impl Challenge {
    pub(super) fn is_stale(&self) -> bool {
        self.0.stale
    }

    pub(super) fn nonce(&self) -> &str {
        &self.0.nonce
    }

    pub(super) fn authorization(
        &mut self,
        username: &str,
        password: &str,
        method: &str,
        request_target: &str,
        body: &str,
    ) -> Result<String, ChallengeError> {
        if username.chars().any(char::is_control) {
            return Err(ChallengeError::Malformed);
        }
        let context = AuthContext::new_with_method(
            username,
            password,
            request_target,
            Some(body.as_bytes()),
            method.into(),
        );
        let mut answer = self
            .0
            .respond(&context)
            .map_err(|_| ChallengeError::Malformed)?;
        let header = if self.0.charset == Charset::UTF8 && !answer.userhash && !username.is_ascii()
        {
            // 散列仍使用原始用户名，由 username* 表达 UTF-8 字节。
            answer.username.clear();
            let serialized = answer.to_string();
            let suffix = serialized
                .strip_prefix("Digest username=\"\"")
                .ok_or(ChallengeError::Malformed)?;
            let mut header = String::from("Digest username*=UTF-8''");
            for byte in username.bytes() {
                if byte.is_ascii_alphanumeric() {
                    header.push(char::from(byte));
                } else {
                    let _ = write!(header, "%{byte:02X}");
                }
            }
            header.push_str(suffix);
            header
        } else {
            answer.to_string()
        };
        if !header.is_ascii() {
            return Err(ChallengeError::Unsupported);
        }
        if !header
            .bytes()
            .all(|byte| matches!(byte, b'\t' | b' '..=b'~'))
        {
            return Err(ChallengeError::Malformed);
        }
        Ok(header)
    }
}

pub(super) fn challenge(headers: &[&str]) -> Result<Option<Challenge>, ChallengeError> {
    let mut chosen = None;
    let mut first_error = None;
    for (scheme, fields) in headers.iter().flat_map(|header| split_challenges(header)) {
        if !scheme.eq_ignore_ascii_case("Digest") {
            continue;
        }
        match parse_digest(fields) {
            Ok(prompt) => {
                let strength = match prompt.algorithm.algo {
                    AlgorithmType::MD5 => 1,
                    AlgorithmType::SHA2_256 => 2,
                    AlgorithmType::SHA2_512_256 => 3,
                };
                if chosen
                    .as_ref()
                    .is_none_or(|(previous, _)| strength > *previous)
                {
                    chosen = Some((strength, Challenge(prompt)));
                }
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    match chosen {
        Some((_, challenge)) => Ok(Some(challenge)),
        None => first_error.map_or(Ok(None), Err),
    }
}

pub(super) fn offers_basic(headers: &[&str]) -> bool {
    headers
        .iter()
        .flat_map(|header| split_challenges(header))
        .any(|(scheme, _)| scheme.eq_ignore_ascii_case("Basic"))
}

fn split_challenges(header: &str) -> Vec<(&str, Vec<&str>)> {
    let mut in_quotes = false;
    let mut escaped = false;
    let mut start = 0;
    let mut challenges: Vec<(&str, Vec<&str>)> = Vec::new();
    let commas = header.char_indices().filter_map(|(index, ch)| {
        match ch {
            _ if escaped => escaped = false,
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => return Some(index),
            _ => {}
        }
        None
    });
    for end in commas.chain(std::iter::once(header.len())) {
        let part = header[start..end].trim();
        start = end + 1;
        if part.is_empty() {
            continue;
        }
        let name_end = part
            .find(|ch: char| ch == '=' || ch.is_ascii_whitespace())
            .unwrap_or(part.len());
        let (name, tail) = part.split_at(name_end);
        let tail = tail.trim_start();
        // 逗号后是 name=value 时，仍属于前一个认证挑战。
        if tail.starts_with('=') {
            if let Some((_, fields)) = challenges.last_mut() {
                fields.push(part);
            }
        } else {
            let fields = if tail.is_empty() {
                Vec::new()
            } else {
                vec![tail]
            };
            challenges.push((name, fields));
        }
    }
    challenges
}

fn parse_digest(fields: Vec<&str>) -> Result<WwwAuthenticateHeader, ChallengeError> {
    use ChallengeError::{Malformed, Unsupported};

    let mut values = HashMap::new();
    for field in fields {
        let (key, value) = field.split_once('=').ok_or(Malformed)?;
        let key = key.trim();
        if !is_token(key)
            || values
                .insert(key.to_ascii_lowercase(), decode_value(value.trim())?)
                .is_some()
        {
            return Err(Malformed);
        }
    }
    let realm = values.remove("realm").ok_or(Malformed)?;
    let nonce = values
        .remove("nonce")
        .filter(|nonce| !nonce.is_empty())
        .ok_or(Malformed)?;
    let algorithm = match values.get("algorithm") {
        Some(value) => {
            let uppercase = value.to_ascii_uppercase();
            let base = uppercase.strip_suffix("-SESS");
            let algorithm = match base.unwrap_or(&uppercase) {
                "MD5" => AlgorithmType::MD5,
                "SHA-256" => AlgorithmType::SHA2_256,
                "SHA-512-256" => AlgorithmType::SHA2_512_256,
                _ => return Err(Unsupported),
            };
            Algorithm::new(algorithm, base.is_some())
        }
        None => Algorithm::default(),
    };
    let qop = values
        .get("qop")
        .map(|value| {
            let supported: Vec<_> = value
                .split(',')
                .filter_map(|entry| {
                    let entry = entry.trim();
                    if entry.eq_ignore_ascii_case("auth-int") {
                        Some(Qop::AUTH_INT)
                    } else if entry.eq_ignore_ascii_case("auth") {
                        Some(Qop::AUTH)
                    } else {
                        None
                    }
                })
                .collect();
            if supported.is_empty() {
                Err(Unsupported)
            } else {
                Ok(supported)
            }
        })
        .transpose()?;
    if algorithm.sess && qop.is_none() {
        return Err(Unsupported);
    }
    let charset = match values.get("charset") {
        Some(value) if value.eq_ignore_ascii_case("UTF-8") => Charset::UTF8,
        Some(_) => return Err(Unsupported),
        None => Charset::ASCII,
    };
    Ok(WwwAuthenticateHeader {
        domain: None,
        realm,
        nonce,
        opaque: values.remove("opaque"),
        stale: values
            .get("stale")
            .is_some_and(|value| value.eq_ignore_ascii_case("true")),
        userhash: values
            .get("userhash")
            .is_some_and(|value| value.eq_ignore_ascii_case("true")),
        algorithm,
        qop,
        charset,
        nc: 0,
    })
}

fn is_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

fn decode_value(value: &str) -> Result<String, ChallengeError> {
    let Some(quoted) = value.strip_prefix('"') else {
        return is_token(value)
            .then(|| value.to_owned())
            .ok_or(ChallengeError::Malformed);
    };
    let mut decoded = String::new();
    let mut chars = quoted.chars();
    while let Some(ch) = chars.next() {
        let ch = match ch {
            '"' if chars.as_str().is_empty() => return Ok(decoded),
            '"' => break,
            '\\' => chars.next().ok_or(ChallengeError::Malformed)?,
            other => other,
        };
        if ch.is_control() && ch != '\t' {
            break;
        }
        decoded.push(ch);
    }
    Err(ChallengeError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_utf8_usernames_and_rejects_invalid_header_values() {
        let mut utf8 = challenge(&[r#"Digest realm="wps", nonce="fresh", charset=UTF-8"#])
            .unwrap()
            .unwrap();
        let header = utf8
            .authorization("用户+1", "secret", "PROPFIND", "/dav", "")
            .unwrap();
        assert!(header.starts_with("Digest username*=UTF-8''%E7%94%A8%E6%88%B7%2B1,"));
        assert_eq!(
            utf8.authorization("user\r\nInjected: field", "secret", "PROPFIND", "/dav", ""),
            Err(ChallengeError::Malformed)
        );

        let mut legacy = challenge(&[r#"Digest realm="wps", nonce="fresh""#])
            .unwrap()
            .unwrap();
        assert_eq!(
            legacy.authorization("用户", "secret", "PROPFIND", "/dav", ""),
            Err(ChallengeError::Unsupported)
        );
    }

    #[test]
    fn selects_the_strongest_digest_from_combined_and_separate_headers() {
        let headers = [
            r#"Basic realm="Digest, Basic", dIgEsT REALM = "wps, \"日历\"", Nonce="legacy", QOP="auth""#,
            r#"Bearer token, Digest realm="work", nonce="strong", algorithm=sha-256, stale=TRUE, qop="future, AUTH", Basic realm="other""#,
        ];
        let selected = challenge(&headers).unwrap().unwrap();
        assert_eq!(selected.nonce(), "strong");
        assert!(selected.is_stale());
        assert_eq!(selected.0.algorithm.algo, AlgorithmType::SHA2_256);
        assert_eq!(selected.0.qop, Some(vec![Qop::AUTH]));
        let fallback = challenge(&headers[..1]).unwrap().unwrap();
        assert_eq!(fallback.0.realm, "wps, \"日历\"");
        assert_eq!(fallback.0.algorithm.algo, AlgorithmType::MD5);
        assert!(offers_basic(&headers));
        assert!(!offers_basic(&[r#"Bearer realm="example, Basic realm""#]));
    }

    #[test]
    fn distinguishes_no_digest_from_malformed_or_unsupported_digest() {
        assert!(challenge(&[r#"Basic realm="example""#]).unwrap().is_none());
        for header in [
            r#"Digest realm="example""#,
            r#"Digest realm="example", nonce="two", Nonce="duplicate""#,
            r#"Digest realm="example", nonce="not closed"#,
        ] {
            assert!(matches!(
                challenge(&[header]),
                Err(ChallengeError::Malformed)
            ));
        }
        for header in [
            r#"Digest realm="example", nonce="one", algorithm=unknown"#,
            r#"Digest realm="example", nonce="one", qop="future""#,
        ] {
            assert!(matches!(
                challenge(&[header]),
                Err(ChallengeError::Unsupported)
            ));
        }
    }

    #[test]
    fn matches_the_rfc_7616_md5_and_sha256_examples() {
        for (algorithm, response) in [
            ("MD5", "8ca523f5e9506fed4657c9700eebdbec"),
            (
                "SHA-256",
                "753927fa0e85d155564e2e272a28d1802ca10daf4496794697cf8db5856cb6c1",
            ),
        ] {
            let value = format!(
                r#"Digest realm="http-auth@example.org", qop="auth", algorithm={algorithm}, nonce="7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v", opaque="FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS""#
            );
            let mut selected = challenge(&[&value]).unwrap().unwrap();
            let mut context = AuthContext::new("Mufasa", "Circle of Life", "/dir/index.html");
            context.set_custom_cnonce("f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ");
            let answer = selected.0.respond(&context).unwrap();
            assert_eq!(answer.response, response);
        }
    }

    #[test]
    fn authorization_uses_the_dav_method_query_and_xml_body() {
        let headers =
            [r#"Digest realm="test", nonce="fresh", qop="auth-int", algorithm=SHA-256-sess"#];
        let mut selected = challenge(&headers).unwrap().unwrap();
        let header = selected
            .authorization("test-user", "secret", "PROPFIND", "/dav?a=b", "<xml/>")
            .unwrap();
        let answer = digest_auth::AuthorizationHeader::parse(&header).unwrap();
        assert_eq!(answer.uri, "/dav?a=b");
        assert_eq!(answer.qop, Some(Qop::AUTH_INT));
        let hash = |value: &str| {
            ring::digest::digest(&ring::digest::SHA256, value.as_bytes())
                .as_ref()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        let cnonce = answer.cnonce.unwrap();
        let ha1 = hash(&format!("{}:fresh:{cnonce}", hash("test-user:test:secret")));
        let ha2 = hash(&format!("PROPFIND:/dav?a=b:{}", hash("<xml/>")));
        assert_eq!(
            answer.response,
            hash(&format!("{ha1}:fresh:00000001:{cnonce}:auth-int:{ha2}"))
        );
    }
}
