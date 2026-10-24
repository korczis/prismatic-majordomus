//! The mesh store, spoken natively: S3 requests signed with AWS Signature Version 4 over a
//! plain HTTP/1.1 connection, every call bounded in time (I3951, ADR 0157).
//!
//! **Why not a crate.** The executable carries no HTTP client and no HMAC crate, and the store
//! needs four requests: a health probe, `HEAD`, `GET` into a file and `PUT` from a file. The
//! signature is a few hashes over strings whose shape AWS publishes with test vectors, which
//! the tests below reproduce byte for byte; a dependency would be larger than the code.
//!
//! **The credential never travels.** `Credential` reads a node's key from the AWS-format file
//! of mode 0600 outside the repository that sccache also reads, and prints itself without the
//! secret. A file anybody else can
//! read is refused, because a key that has leaked on its own machine has leaked. Only the
//! access key id — which names the node's identity in the store and is no secret — appears in
//! an answer.
//!
//! **Plain HTTP only, on purpose.** The store is reached on the mesh's private network, and
//! the endpoint's scheme is checked: `https://` is refused with its cause rather than spoken
//! in clear by mistake. TLS joins when a provider needs it.
//!
//! **Bounded.** Connecting, every read and every write carry the caller's budget. A store that
//! does not answer within it is `Probe::Unknown` — never "down" — and a transfer that
//! stalls fails, so the caller falls back to doing the work itself.

// The client's first caller is `mesh build-cache ensure`, the next change of this milestone
// (I3952); until then only the tests below build one.
#![allow(dead_code)]

use std::fmt;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use sha2::{Digest, Sha256};

/// The SHA-256 of the empty string: the payload hash of every request without a body.
pub(crate) const EMPTY_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// What a PUT declares instead of its body's hash: the body is streamed, not read twice.
pub(crate) const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";

pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[usize::from(b >> 4)] as char);
        out.push(DIGITS[usize::from(b & 0xf)] as char);
    }
    out
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// HMAC-SHA256 (RFC 2104): the only keyed primitive SigV4 uses.
pub(crate) fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut block = [0u8; BLOCK];
    if key.len() > BLOCK {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(block.map(|b| b ^ 0x36));
    inner.update(message);
    let mut outer = Sha256::new();
    outer.update(block.map(|b| b ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

/// A node's identity in the store. `Debug` and `Display` never show the secret.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Credential {
    pub(crate) access_key: String,
    secret_key: String,
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Credential({}, secret withheld)", self.access_key)
    }
}

/// Why a credential could not be read. Names the file and the cause, never a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CredentialError {
    /// No file: this node holds no credential, which is a state, not a fault.
    Absent,
    /// The file is readable by its group or by others.
    Exposed(u32),
    /// The file lacks one of the two keys.
    Incomplete(&'static str),
    /// The file could not be read.
    Unreadable(String),
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CredentialError::Absent => write!(f, "this node holds no store credential"),
            CredentialError::Exposed(mode) => write!(
                f,
                "the credential file has mode {mode:o}; it must be readable by its owner alone (chmod 600)"
            ),
            CredentialError::Incomplete(key) => write!(f, "the credential file has no {key}"),
            CredentialError::Unreadable(cause) => {
                write!(f, "the credential file cannot be read: {cause}")
            }
        }
    }
}

impl Credential {
    pub(crate) fn new(access_key: &str, secret_key: &str) -> Credential {
        Credential {
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
        }
    }

    /// The profile a node's credential is filed under, in the AWS-format file sccache reads
    /// through `AWS_SHARED_CREDENTIALS_FILE`.
    pub(crate) const PROFILE: &'static str = "mesh-build-cache";

    /// Read the [`Credential::PROFILE`] section of an AWS-format credentials file, refusing a
    /// file that anybody but its owner can read. The secret is never passed through the
    /// environment: an exported value is readable in every child's environment.
    pub(crate) fn read(path: &Path) -> Result<Credential, CredentialError> {
        let meta = match std::fs::metadata(path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(CredentialError::Absent),
            Err(e) => return Err(CredentialError::Unreadable(e.kind().to_string())),
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = meta.permissions().mode() & 0o777;
            if mode & 0o077 != 0 {
                return Err(CredentialError::Exposed(mode));
            }
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| CredentialError::Unreadable(e.kind().to_string()))?;
        let mut in_profile = false;
        let (mut access, mut secret) = (None, None);
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                in_profile = line == format!("[{}]", Self::PROFILE);
                continue;
            }
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            if !in_profile {
                continue;
            }
            let value = value.trim().to_string();
            match name.trim() {
                "aws_access_key_id" if !value.is_empty() => access = Some(value),
                "aws_secret_access_key" if !value.is_empty() => secret = Some(value),
                _ => {}
            }
        }
        let access = access.ok_or(CredentialError::Incomplete("aws_access_key_id"))?;
        let secret = secret.ok_or(CredentialError::Incomplete("aws_secret_access_key"))?;
        Ok(Credential::new(&access, &secret))
    }
}

/// `YYYYMMDDTHHMMSSZ`, the timestamp SigV4 signs.
pub(crate) fn amz_date(t: SystemTime) -> String {
    crate::peers::rfc3339(t).replace(['-', ':'], "")
}

/// Percent-encode a path for the canonical request: every byte but the unreserved set and
/// the separator, uppercase hex, as S3 expects (encoded once).
pub(crate) fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// One request to sign: method, the already encoded path, the canonical query string, the
/// headers to sign (any case; `host`, `x-amz-date` and `x-amz-content-sha256` among them) and
/// the payload hash.
pub(crate) struct Signable<'a> {
    pub(crate) method: &'a str,
    pub(crate) path: &'a str,
    pub(crate) query: &'a str,
    pub(crate) headers: &'a [(String, String)],
    pub(crate) payload_sha256: &'a str,
}

/// The `Authorization` header of a request, and the canonical request it signed (returned
/// so the tests can hold it to AWS's published intermediate values).
pub(crate) fn authorization(
    request: &Signable<'_>,
    credential: &Credential,
    region: &str,
    amz_date: &str,
) -> (String, String) {
    // SigV4 orders the signed headers by their lowercase names, byte by byte: a map's order
    let headers: std::collections::BTreeMap<String, String> = request
        .headers
        .iter()
        .map(|(k, v)| {
            (
                k.to_ascii_lowercase(),
                v.split_whitespace().collect::<Vec<_>>().join(" "),
            )
        })
        .collect();
    let canonical_headers: String = headers.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    let signed_headers = headers
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(";");
    let canonical = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        request.method,
        request.path,
        request.query,
        canonical_headers,
        signed_headers,
        request.payload_sha256
    );
    let date = &amz_date[..8];
    let scope = format!("{date}/{region}/s3/aws4_request");
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical.as_bytes())
    );
    let k_date = hmac_sha256(
        format!("AWS4{}", credential.secret_key).as_bytes(),
        date.as_bytes(),
    );
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, b"s3");
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    let signature = hex(&hmac_sha256(&k_signing, to_sign.as_bytes()));
    (
        format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            credential.access_key
        ),
        canonical,
    )
}

/// Where a store answers: `host:port`, from an `http://` endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Endpoint {
    pub(crate) host: String,
}

impl Endpoint {
    /// Parse `http://host:port[/]`, or `host:port` as `http://host:port`. Any other scheme, a
    /// path or a missing port is refused with its cause.
    pub(crate) fn parse(url: &str) -> Result<Endpoint, String> {
        let url = super::normalise_endpoint(url);
        let url = url.as_str();
        let rest = url.strip_prefix("http://").ok_or_else(|| {
            format!("{url}: only http:// endpoints on the mesh's network are spoken")
        })?;
        let host = rest.trim_end_matches('/');
        if host.is_empty() || host.contains('/') {
            return Err(format!(
                "{url}: an endpoint is a host and a port, without a path"
            ));
        }
        if !host
            .rsplit_once(':')
            .is_some_and(|(_, p)| p.parse::<u16>().is_ok())
        {
            return Err(format!("{url}: the endpoint names no port"));
        }
        Ok(Endpoint {
            host: host.to_string(),
        })
    }

    fn address(&self) -> io::Result<SocketAddr> {
        first_address(self.host.to_socket_addrs()?)
    }

    fn connect(&self, budget: Duration) -> io::Result<TcpStream> {
        let stream = TcpStream::connect_timeout(&self.address()?, budget)?;
        bounded(stream, budget)
    }
}

/// `stream` with every read and write bounded by `budget`, and no delay on small writes; the
/// first setting refused is the error (a zero budget is the one a timeout refuses).
fn bounded(stream: TcpStream, budget: Duration) -> io::Result<TcpStream> {
    stream
        .set_read_timeout(Some(budget))
        .and_then(|()| stream.set_write_timeout(Some(budget)))
        .and_then(|()| stream.set_nodelay(true))
        .map(|()| stream)
}

/// The first address a name resolved to; a name that resolved to none is not found.
fn first_address(mut addresses: impl Iterator<Item = SocketAddr>) -> io::Result<SocketAddr> {
    addresses
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no address"))
}

/// What a health probe learned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Probe {
    /// The store answered its liveness path with success, after this many milliseconds.
    Healthy(u64),
    /// The store answered, and not with success.
    Unhealthy(u16),
    /// Nothing answered within the budget: not known to be down, only not known.
    Unknown,
}

/// A response's status line and the headers this client reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Head {
    pub(crate) status: u16,
    pub(crate) content_length: Option<u64>,
}

fn read_head(reader: &mut impl BufRead) -> io::Result<Head> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let status = line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no HTTP status line"))?;
    let mut content_length = None;
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().ok();
            }
        }
    }
    Ok(Head {
        status,
        content_length,
    })
}

/// A client of one bucket of one store.
#[derive(Debug, Clone)]
pub(crate) struct Store {
    pub(crate) endpoint: Endpoint,
    pub(crate) bucket: String,
    pub(crate) region: String,
    pub(crate) credential: Credential,
    pub(crate) budget: Duration,
}

impl Store {
    fn object_path(&self, key: &str) -> String {
        encode_path(&format!("/{}/{}", self.bucket, key.trim_start_matches('/')))
    }

    fn request(
        &self,
        method: &str,
        key: &str,
        payload_sha256: &str,
        length: Option<u64>,
        now: SystemTime,
    ) -> String {
        let path = self.object_path(key);
        let date = amz_date(now);
        let headers = vec![
            ("host".to_string(), self.endpoint.host.clone()),
            (
                "x-amz-content-sha256".to_string(),
                payload_sha256.to_string(),
            ),
            ("x-amz-date".to_string(), date.clone()),
        ];
        let (auth, _) = authorization(
            &Signable {
                method,
                path: &path,
                query: "",
                headers: &headers,
                payload_sha256,
            },
            &self.credential,
            &self.region,
            &date,
        );
        let mut text = format!("{method} {path} HTTP/1.1\r\n");
        for (k, v) in &headers {
            text.push_str(&format!("{k}: {v}\r\n"));
        }
        text.push_str(&format!("authorization: {auth}\r\n"));
        if let Some(n) = length {
            text.push_str(&format!("content-length: {n}\r\n"));
        }
        text.push_str("connection: close\r\n\r\n");
        text
    }

    /// The object's size, or `None` when the store has no such object.
    pub(crate) fn head(&self, key: &str) -> io::Result<Option<u64>> {
        let mut stream = self.endpoint.connect(self.budget)?;
        stream.write_all(
            self.request("HEAD", key, EMPTY_SHA256, None, SystemTime::now())
                .as_bytes(),
        )?;
        let head = read_head(&mut BufReader::new(stream))?;
        match head.status {
            200 => Ok(Some(head.content_length.unwrap_or(0))),
            404 => Ok(None),
            status => Err(refused(status)),
        }
    }

    /// Fetch an object into `dest`. `Ok(false)` when the store has no such object; a body
    /// shorter than its declared length is an error, and `dest` is then removed.
    pub(crate) fn get(&self, key: &str, dest: &Path) -> io::Result<bool> {
        let mut stream = self.endpoint.connect(self.budget)?;
        stream.write_all(
            self.request("GET", key, EMPTY_SHA256, None, SystemTime::now())
                .as_bytes(),
        )?;
        let mut reader = BufReader::with_capacity(1 << 20, stream);
        let head = read_head(&mut reader)?;
        match head.status {
            200 => {}
            404 => return Ok(false),
            status => return Err(refused(status)),
        }
        let copied = File::create(dest).and_then(|mut file| io::copy(&mut reader, &mut file));
        match (copied, head.content_length) {
            (Ok(n), Some(expected)) if n == expected => Ok(true),
            (Ok(_), None) => Ok(true),
            (Ok(n), Some(expected)) => {
                let _ = std::fs::remove_file(dest);
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!("the object was cut short: {n} of {expected} bytes"),
                ))
            }
            (Err(e), _) => {
                let _ = std::fs::remove_file(dest);
                Err(e)
            }
        }
    }

    /// Store `source` under `key`. The body is streamed; its hash is not signed.
    pub(crate) fn put(&self, key: &str, source: &Path) -> io::Result<()> {
        let (mut file, length) =
            File::open(source).and_then(|f| f.metadata().map(|m| (f, m.len())))?;
        let mut stream = self.endpoint.connect(self.budget)?;
        stream.write_all(
            self.request(
                "PUT",
                key,
                UNSIGNED_PAYLOAD,
                Some(length),
                SystemTime::now(),
            )
            .as_bytes(),
        )?;
        // a TcpStream writes through: there is nothing to flush before the answer is read
        io::copy(&mut file, &mut stream)?;
        let head = read_head(&mut BufReader::new(stream))?;
        match head.status {
            200 => Ok(()),
            status => Err(refused(status)),
        }
    }
}

fn refused(status: u16) -> io::Error {
    let what = match status {
        403 => "the store refused this node's credential (403)".to_string(),
        507 | 413 => format!("the store has no room for the object ({status})"),
        _ => format!("the store answered {status}"),
    };
    io::Error::other(what)
}

/// Ask a store's unauthenticated liveness path, within `budget`.
pub(crate) fn probe(endpoint: &Endpoint, budget: Duration) -> Probe {
    let started = Instant::now();
    let answer = endpoint.connect(budget).and_then(|mut stream| {
        stream
            .write_all(
                format!(
                    "GET /minio/health/live HTTP/1.1\r\nhost: {}\r\nconnection: close\r\n\r\n",
                    endpoint.host
                )
                .as_bytes(),
            )
            .and_then(|()| read_head(&mut BufReader::new(stream)))
            .map(|head| head.status)
    });
    match answer {
        Ok(200) => Probe::Healthy(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
        Ok(status) => Probe::Unhealthy(status),
        Err(_) => Probe::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mj-s3-{}-{}-{name}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn hmac_matches_rfc_4231() {
        // RFC 4231, test case 1
        let mac = hmac_sha256(&[0x0b; 20], b"Hi There");
        assert_eq!(
            hex(&mac),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        // test case 6: a key longer than the block is hashed first
        let mac = hmac_sha256(
            &[0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First",
        );
        assert_eq!(
            hex(&mac),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn signature_matches_the_aws_get_object_example() {
        // docs.aws.amazon.com, "Signature Calculations for the Authorization Header",
        // Example: GET Object.
        let credential = Credential::new(
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        );
        let headers = vec![
            (
                "Host".to_string(),
                "examplebucket.s3.amazonaws.com".to_string(),
            ),
            ("Range".to_string(), "bytes=0-9".to_string()),
            ("x-amz-content-sha256".to_string(), EMPTY_SHA256.to_string()),
            ("x-amz-date".to_string(), "20130524T000000Z".to_string()),
        ];
        let request = Signable {
            method: "GET",
            path: "/test.txt",
            query: "",
            headers: &headers,
            payload_sha256: EMPTY_SHA256,
        };
        let (auth, canonical) =
            authorization(&request, &credential, "us-east-1", "20130524T000000Z");
        assert_eq!(
            sha256_hex(canonical.as_bytes()),
            "7344ae5b7ee6c3e7e6b0fe0640412a37625d1fbfff95c48bbb2dc43964946972"
        );
        assert_eq!(
            auth,
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
             SignedHeaders=host;range;x-amz-content-sha256;x-amz-date, \
             Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
        );
    }

    #[test]
    fn a_credential_never_prints_its_secret() {
        let c = Credential::new("mesh-cache-n1", "s3cr3t-value");
        assert!(!format!("{c:?}").contains("s3cr3t"));
        assert!(format!("{c:?}").contains("mesh-cache-n1"));
    }

    #[test]
    fn a_credential_file_is_read_and_its_exposure_refused() {
        let dir = temp("cred");
        let path = dir.join("credentials");
        assert_eq!(Credential::read(&path), Err(CredentialError::Absent));
        std::fs::write(
            &path,
            "[default]\naws_access_key_id = other\naws_secret_access_key = no\n\n[mesh-build-cache]\naws_access_key_id = mesh-cache-n1\naws_secret_access_key = abc\nnot a pair\nregion = x\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            let refused = Credential::read(&path).unwrap_err();
            assert_eq!(refused, CredentialError::Exposed(0o644));
            assert!(refused.to_string().contains("chmod 600"));
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert_eq!(
            Credential::read(&path).unwrap(),
            Credential::new("mesh-cache-n1", "abc")
        );
        std::fs::write(&path, "[mesh-build-cache]\naws_access_key_id = x\n").unwrap();
        assert_eq!(
            Credential::read(&path),
            Err(CredentialError::Incomplete("aws_secret_access_key"))
        );
        std::fs::write(&path, "[default]\naws_access_key_id = x\n").unwrap();
        assert_eq!(
            Credential::read(&path),
            Err(CredentialError::Incomplete("aws_access_key_id"))
        );
        assert!(CredentialError::Absent
            .to_string()
            .contains("no store credential"));
        assert!(CredentialError::Incomplete("K")
            .to_string()
            .contains("no K"));
        assert!(CredentialError::Unreadable("x".into())
            .to_string()
            .contains("cannot be read"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn endpoints_are_plain_http_host_and_port() {
        assert_eq!(
            Endpoint::parse("http://10.0.0.1:9000/").unwrap().host,
            "10.0.0.1:9000"
        );
        assert!(Endpoint::parse("https://h:9000")
            .unwrap_err()
            .contains("only http://"));
        assert!(Endpoint::parse("http://h:9000/bucket")
            .unwrap_err()
            .contains("without a path"));
        assert!(Endpoint::parse("http://h").unwrap_err().contains("no port"));
        // no scheme is the same endpoint as `http://`: one normaliser, shared with the probe
        assert_eq!(
            Endpoint::parse("10.0.0.1:9000").unwrap(),
            Endpoint::parse("http://10.0.0.1:9000").unwrap()
        );
        assert!(Endpoint::parse("h").unwrap_err().contains("no port"));
        assert!(Endpoint::parse("http://")
            .unwrap_err()
            .contains("without a path"));
    }

    #[test]
    fn paths_are_encoded_once() {
        assert_eq!(
            encode_path("/b/rust/x86_64/a b+c"),
            "/b/rust/x86_64/a%20b%2Bc"
        );
        assert_eq!(amz_date(SystemTime::UNIX_EPOCH), "19700101T000000Z");
    }

    #[test]
    fn a_response_head_is_read() {
        let mut r =
            Cursor::new(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nX: y\r\n\r\nhello".to_vec());
        assert_eq!(
            read_head(&mut r).unwrap(),
            Head {
                status: 200,
                content_length: Some(5)
            }
        );
        assert!(read_head(&mut Cursor::new(b"garbage\r\n".to_vec())).is_err());
    }

    /// A one-shot server: answers each accepted connection with the next canned response
    /// and hands the request it read back to the test.
    fn serve(responses: Vec<Vec<u8>>) -> (Endpoint, mpsc::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = listener.local_addr().unwrap().to_string();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for response in responses {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_millis(300)))
                    .unwrap();
                let mut seen = Vec::new();
                let mut buf = [0u8; 65536];
                while let Ok(n) = socket.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    seen.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&seen);
                    if let Some(end) = text.find("\r\n\r\n") {
                        let declared = text[..end]
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length: "))
                            .and_then(|n| n.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        if seen.len() >= end + 4 + declared {
                            break;
                        }
                    }
                }
                socket.write_all(&response).unwrap();
                tx.send(seen).unwrap();
            }
        });
        (Endpoint { host }, rx)
    }

    fn store(endpoint: Endpoint) -> Store {
        Store {
            endpoint,
            bucket: "mesh-build-cache".into(),
            region: "us-east-1".into(),
            credential: Credential::new("mesh-cache-test", "secret-of-test"),
            budget: Duration::from_secs(2),
        }
    }

    #[test]
    fn get_put_and_head_speak_signed_requests() {
        let dir = temp("rw");
        let (endpoint, seen) = serve(vec![
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello".to_vec(),
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 42\r\n\r\n".to_vec(),
            b"HTTP/1.1 404 Not Found\r\n\r\n".to_vec(),
        ]);
        let s = store(endpoint);
        let dest = dir.join("got");
        assert!(s.get("rust/a b", &dest).unwrap());
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello");
        let request = String::from_utf8(seen.recv().unwrap()).unwrap();
        assert!(request.starts_with("GET /mesh-build-cache/rust/a%20b HTTP/1.1\r\n"));
        assert!(request.contains("authorization: AWS4-HMAC-SHA256 Credential=mesh-cache-test/"));
        assert!(!request.contains("secret-of-test"));

        assert!(!s.get("missing", &dir.join("none")).unwrap());
        let _ = seen.recv().unwrap();

        let source = dir.join("up");
        std::fs::write(&source, b"payload").unwrap();
        s.put("k", &source).unwrap();
        let request = String::from_utf8(seen.recv().unwrap()).unwrap();
        assert!(request.contains("x-amz-content-sha256: UNSIGNED-PAYLOAD"));
        assert!(request.contains("content-length: 7"));
        assert!(request.ends_with("payload"));

        assert_eq!(s.head("k").unwrap(), Some(42));
        let _ = seen.recv().unwrap();
        assert_eq!(s.head("absent").unwrap(), None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refusals_and_short_bodies_are_errors() {
        let dir = temp("err");
        let (endpoint, _seen) = serve(vec![
            b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n".to_vec(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nshort".to_vec(),
            b"HTTP/1.1 507 Insufficient Storage\r\n\r\n".to_vec(),
            b"HTTP/1.1 500 Internal\r\n\r\n".to_vec(),
        ]);
        let s = store(endpoint);
        let e = s.get("k", &dir.join("a")).unwrap_err();
        assert!(e.to_string().contains("refused this node's credential"));
        let e = s.get("k", &dir.join("b")).unwrap_err();
        assert!(e.to_string().contains("cut short: 5 of 10"));
        assert!(!dir.join("b").exists());
        let source = dir.join("up");
        std::fs::write(&source, b"x").unwrap();
        assert!(s
            .put("k", &source)
            .unwrap_err()
            .to_string()
            .contains("no room"));
        assert!(s
            .head("k")
            .unwrap_err()
            .to_string()
            .contains("answered 500"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_probe_is_healthy_unhealthy_or_unknown() {
        let (endpoint, _seen) = serve(vec![
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
            b"HTTP/1.1 503 Unavailable\r\n\r\n".to_vec(),
        ]);
        assert!(matches!(
            probe(&endpoint, Duration::from_secs(2)),
            Probe::Healthy(_)
        ));
        assert_eq!(
            probe(&endpoint, Duration::from_secs(2)),
            Probe::Unhealthy(503)
        );
        // a port nothing listens on
        let closed = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .to_string();
        assert_eq!(
            probe(&Endpoint { host: closed }, Duration::from_millis(200)),
            Probe::Unknown
        );
    }

    /// A server that accepts one connection and holds it, reading nothing and answering
    /// nothing, until `hold` has passed.
    fn mute(hold: Duration) -> Endpoint {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = listener.local_addr().unwrap().to_string();
        thread::spawn(move || {
            if let Ok((socket, _)) = listener.accept() {
                thread::sleep(hold);
                drop(socket);
            }
        });
        Endpoint { host }
    }

    fn closed_port() -> Endpoint {
        let host = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .to_string();
        Endpoint { host }
    }

    fn quick(endpoint: Endpoint) -> Store {
        Store {
            budget: Duration::from_millis(300),
            ..store(endpoint)
        }
    }

    fn is_root() -> bool {
        // SAFETY: geteuid reads the effective user id and cannot fail.
        unsafe { libc::geteuid() == 0 }
    }

    #[test]
    fn a_credential_path_that_cannot_be_inspected_or_read_is_unreadable_not_absent() {
        let dir = temp("credunread");
        let file = dir.join("plain");
        std::fs::write(&file, "x").unwrap();
        assert!(matches!(
            Credential::read(&file.join("credentials")),
            Err(CredentialError::Unreadable(_))
        ));
        if !is_root() {
            use std::os::unix::fs::PermissionsExt;
            let path = dir.join("credentials");
            std::fs::write(&path, "[mesh-build-cache]\n").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o200)).unwrap();
            assert!(matches!(
                Credential::read(&path),
                Err(CredentialError::Unreadable(_))
            ));
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_name_that_resolves_to_nothing_or_is_no_address_cannot_be_connected() {
        let none = std::iter::empty::<SocketAddr>();
        assert_eq!(
            first_address(none).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        let one: SocketAddr = "127.0.0.1:9".parse().unwrap();
        assert_eq!(first_address(std::iter::once(one)).unwrap(), one);
        let malformed = Endpoint {
            host: "no-port-at-all".into(),
        };
        assert!(malformed.connect(Duration::from_millis(100)).is_err());
        assert!(
            closed_port().connect(Duration::from_millis(200)).is_err(),
            "nothing listens"
        );
    }

    #[test]
    fn a_response_head_that_breaks_off_is_an_error_and_a_header_without_a_colon_is_skipped() {
        struct Broken(Vec<Vec<u8>>);
        impl Read for Broken {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.0.is_empty() {
                    return Err(io::Error::other("connection lost"));
                }
                let next = self.0.remove(0);
                buf[..next.len()].copy_from_slice(&next);
                Ok(next.len())
            }
        }
        // nothing arrives at all
        assert!(read_head(&mut BufReader::new(Broken(Vec::new()))).is_err());
        // the status line arrives, then the connection drops inside the headers
        let cut = Broken(vec![b"HTTP/1.1 200 OK\r\n".to_vec()]);
        assert!(read_head(&mut BufReader::new(cut)).is_err());
        // a header line with no colon is ignored; the next one still counts
        let mut odd =
            Cursor::new(b"HTTP/1.1 200 OK\r\nnot a header\r\ncontent-length: 3\r\n\r\n".to_vec());
        assert_eq!(
            read_head(&mut odd).unwrap(),
            Head {
                status: 200,
                content_length: Some(3)
            }
        );
    }

    #[test]
    fn a_store_that_is_down_or_silent_fails_every_call_within_the_budget() {
        let dir = temp("down");
        let source = dir.join("up");
        std::fs::write(&source, b"payload").unwrap();
        let down = quick(closed_port());
        assert!(down.head("k").is_err());
        assert!(down.get("k", &dir.join("g")).is_err());
        assert!(down.put("k", &source).is_err());
        assert!(
            quick(closed_port()).put("k", &dir.join("missing")).is_err(),
            "a source that is not there is not sent"
        );
        // connected, but nothing answers: the read gives up
        assert!(quick(mute(Duration::from_secs(1))).head("k").is_err());
        assert!(quick(mute(Duration::from_secs(1)))
            .get("k", &dir.join("g2"))
            .is_err());
        assert!(quick(mute(Duration::from_secs(1)))
            .put("k", &source)
            .is_err());
        assert_eq!(
            probe(&mute(Duration::from_secs(1)), Duration::from_millis(300)),
            Probe::Unknown
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_request_the_store_never_reads_times_out_on_the_write() {
        let dir = temp("unread");
        let source = dir.join("up");
        std::fs::write(&source, b"payload").unwrap();
        // a key far larger than any socket buffer cannot be sent to a peer that never reads
        let huge = "k".repeat(32 << 20);
        assert!(quick(mute(Duration::from_secs(2))).head(&huge).is_err());
        assert!(quick(mute(Duration::from_secs(2)))
            .get(&huge, &dir.join("g"))
            .is_err());
        assert!(quick(mute(Duration::from_secs(2)))
            .put(&huge, &source)
            .is_err());
        // a body far larger than any socket buffer: the header goes, the body stalls
        let big = dir.join("big");
        std::fs::write(&big, vec![0u8; 32 << 20]).unwrap();
        assert!(quick(mute(Duration::from_secs(2))).put("k", &big).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_object_without_a_declared_length_is_taken_whole_and_an_unwritable_destination_fails() {
        let dir = temp("nolen");
        let (endpoint, _seen) = serve(vec![
            b"HTTP/1.1 200 OK\r\n\r\nwhole".to_vec(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello".to_vec(),
        ]);
        let s = store(endpoint);
        let dest = dir.join("got");
        assert!(s.get("k", &dest).unwrap());
        assert_eq!(std::fs::read(&dest).unwrap(), b"whole");
        let nowhere = dir.join("no/such/dir/got");
        assert!(s.get("k", &nowhere).is_err());
        assert!(!nowhere.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_stream_is_bounded_by_its_budget_and_a_zero_budget_is_refused() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let at = listener.local_addr().unwrap();
        let stream = bounded(TcpStream::connect(at).unwrap(), Duration::from_secs(3)).unwrap();
        assert_eq!(stream.read_timeout().unwrap(), Some(Duration::from_secs(3)));
        assert_eq!(
            stream.write_timeout().unwrap(),
            Some(Duration::from_secs(3))
        );
        assert!(stream.nodelay().unwrap());
        let zero = bounded(TcpStream::connect(at).unwrap(), Duration::ZERO);
        assert_eq!(zero.unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }
}
