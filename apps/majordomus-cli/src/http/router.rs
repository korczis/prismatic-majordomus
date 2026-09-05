//! Routing and binding, transport-neutral: a request in, a response out. Every route
//! under `/api/v1/` is a capability with an HTTP exposure; the infrastructure routes
//! (`/`, `/openapi.json`, `/docs`, and `/mcp` when the router serves a shared server) are
//! the projection's own and are documented as such. Nothing else exists.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::capability::{CapabilityError, Context, HttpMethod};

use super::mcp::McpEndpoint;
use super::{openapi, swagger};

/// A request as the router sees it: method, path without query, decoded query pairs,
/// headers, and the body.
#[derive(Debug, Clone)]
pub struct Request {
    /// `GET`, `POST`, ... as received (a `HEAD` arrives as `GET`).
    pub method: String,
    /// The path without the query string.
    pub path: String,
    /// The query pairs, percent-decoded, in order.
    pub query: Vec<(String, String)>,
    /// Header names and values as received; looked up case-insensitively.
    pub headers: Vec<(String, String)>,
    /// The body, raw.
    pub body: Vec<u8>,
}

impl Request {
    /// Split a request target (`/api/v1/objects?kind=rule`) into path and decoded pairs.
    ///
    /// ```
    /// use majordomus_cli::http::Request;
    /// let r = Request::parse_target("GET", "/api/v1/search?query=Git%20is&limit=2", vec![]);
    /// assert_eq!(r.path, "/api/v1/search");
    /// assert_eq!(r.query, vec![("query".to_string(), "Git is".to_string()), ("limit".to_string(), "2".to_string())]);
    /// ```
    pub fn parse_target(method: &str, target: &str, body: Vec<u8>) -> Self {
        let (path, query) = match target.split_once('?') {
            Some((p, q)) => (p.to_string(), q),
            None => (target.to_string(), ""),
        };
        let query = query
            .split('&')
            .filter(|s| !s.is_empty())
            .map(|pair| {
                let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                (percent_decode(k), percent_decode(v))
            })
            .collect();
        Request {
            method: method.to_string(),
            path,
            query,
            headers: Vec::new(),
            body,
        }
    }

    /// The same request with its headers.
    pub fn with_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.headers = headers;
        self
    }

    /// A header value, by case-insensitive name.
    ///
    /// ```
    /// use majordomus_cli::http::Request;
    /// let r = Request::parse_target("POST", "/mcp", vec![])
    ///     .with_headers(vec![("Mcp-Session-Id".into(), "abc".into())]);
    /// assert_eq!(r.header("mcp-session-id"), Some("abc"));
    /// assert_eq!(r.header("x-none"), None);
    /// ```
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A response as the router produces it; the server adds the wire.
pub struct Response {
    /// The HTTP status.
    pub status: u16,
    /// The `Content-Type` value.
    pub content_type: &'static str,
    /// The body.
    pub body: String,
    /// Further headers (`Mcp-Session-Id`); the server adds `Content-Length` and `Cache-Control`.
    pub headers: Vec<(String, String)>,
}

impl Response {
    /// A response with no further headers.
    pub fn new(status: u16, content_type: &'static str, body: String) -> Self {
        Response {
            status,
            content_type,
            body,
            headers: Vec::new(),
        }
    }

    /// The JSON error body every failure of the projection uses.
    pub fn error(status: u16, code: &str, message: &str) -> Self {
        error_response(status, code, message)
    }
}

/// The body of every error response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ErrorBody {
    /// The one error.
    pub error: ErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// What went wrong, as the HTTP projection names it.
pub struct ErrorDetail {
    /// `invalid_input`, `not_found`, `refused`, `internal`, `method_not_allowed`.
    pub code: String,
    /// The reason, for a person.
    pub message: String,
}

#[derive(Clone)]
/// Routes requests to capabilities by the registry's HTTP exposures, and serves the
/// projection's own routes. Cheap to clone: every worker thread holds one.
pub struct Router {
    ctx: Arc<Context>,
    version: &'static str,
    /// The OpenAPI document, rendered once: the registry is immutable for the process.
    openapi: Arc<std::sync::OnceLock<Result<String, String>>>,
    /// MCP over HTTP at `/mcp`, when this router serves a shared server.
    mcp: Option<Arc<McpEndpoint>>,
}

impl Router {
    /// A router over a loaded context, without `/mcp`.
    pub fn new(ctx: Arc<Context>, version: &'static str) -> Self {
        crate::perf::Counters::bump(&crate::perf::COUNTERS.http_projection_builds);
        Router {
            ctx,
            version,
            openapi: Arc::new(std::sync::OnceLock::new()),
            mcp: None,
        }
    }

    /// The same router, serving MCP over HTTP at `/mcp` through `endpoint`.
    pub fn with_mcp(mut self, endpoint: Arc<McpEndpoint>) -> Self {
        self.mcp = Some(endpoint);
        self
    }

    fn openapi(&self) -> Response {
        let rendered = self.openapi.get_or_init(|| {
            openapi::document(&self.ctx.registry, self.version).map(|d| openapi::render(&d))
        });
        match rendered {
            Ok(text) => Response::new(200, "application/json", text.clone()),
            Err(e) => error_response(500, "internal", e),
        }
    }

    /// Answer one request: an infrastructure route, `/mcp`, or a capability.
    pub fn handle(&self, req: &Request) -> Response {
        match (req.method.as_str(), req.path.as_str()) {
            ("GET", "/") => {
                let mut index = json!({
                    "name": "majordomus",
                    "version": self.version,
                    "root": self.ctx.index.repository.root,
                    "openapi": "/openapi.json",
                    "docs": swagger::DOCS_PATH,
                    "capabilities": "/api/v1/capabilities",
                    "peers": "/api/v1/peers",
                });
                if self.mcp.is_some() {
                    index["mcp"] = json!(super::mcp::PATH);
                }
                json_response(200, &index)
            }
            ("GET", "/openapi.json") => self.openapi(),
            ("GET", "/docs") => Response::new(
                200,
                "text/html; charset=utf-8",
                swagger::page().to_string(),
            ),
            (_, "/mcp") => match &self.mcp {
                Some(endpoint) => endpoint.handle(req),
                None => error_response(
                    404,
                    "not_found",
                    "this server serves no MCP over HTTP; `majordomus mcp` and `majordomus serve` do, at /mcp",
                ),
            },
            _ => self.capability(req),
        }
    }

    fn capability(&self, req: &Request) -> Response {
        let Some(method) = HttpMethod::parse(&req.method) else {
            return error_response(
                405,
                "method_not_allowed",
                &format!("method {} is not served", req.method),
            );
        };
        let Some(c) = self.ctx.registry.by_http(method, &req.path) else {
            let other_method = [HttpMethod::Get, HttpMethod::Post]
                .into_iter()
                .filter(|m| *m != method)
                .any(|m| self.ctx.registry.by_http(m, &req.path).is_some());
            return if other_method {
                error_response(
                    405,
                    "method_not_allowed",
                    &format!(
                        "{} {} is not a route; another method is",
                        req.method, req.path
                    ),
                )
            } else {
                error_response(
                    404,
                    "not_found",
                    &format!(
                        "no route {} {}; the routes are listed at /openapi.json",
                        req.method, req.path
                    ),
                )
            };
        };
        let input = match method {
            HttpMethod::Get => {
                let (props, _) = c.input.properties();
                let mut obj = serde_json::Map::new();
                for (k, raw) in &req.query {
                    let property = props
                        .iter()
                        .find(|(n, _)| n == k)
                        .map(|(_, s)| s.clone())
                        .unwrap_or(Value::Null);
                    match crate::capability::schema::coerce(&property, raw) {
                        Ok(v) => {
                            obj.insert(k.clone(), v);
                        }
                        Err(e) => {
                            return error_response(
                                400,
                                "invalid_input",
                                &format!("parameter '{k}': {e}"),
                            )
                        }
                    }
                }
                Value::Object(obj)
            }
            HttpMethod::Post => {
                if req.body.iter().all(u8::is_ascii_whitespace) {
                    json!({})
                } else {
                    match serde_json::from_slice::<Value>(&req.body) {
                        Ok(v) if v.is_object() => v,
                        Ok(_) => {
                            return error_response(
                                400,
                                "invalid_input",
                                "the body must be a JSON object",
                            )
                        }
                        Err(e) => {
                            return error_response(
                                400,
                                "invalid_input",
                                &format!("the body is not JSON: {e}"),
                            )
                        }
                    }
                }
            }
        };
        // one line per request is debug: a client that does not drain stderr must not be
        // able to wedge the workers on a full pipe at the default level
        tracing::debug!(capability_id = %c.id, route = %format!("{} {}", req.method, req.path), "http");
        match self.ctx.execute(c.id.as_str(), input) {
            Ok(v) => json_response(200, &v),
            Err(CapabilityError::InvalidInput(m)) => error_response(400, "invalid_input", &m),
            Err(CapabilityError::NotFound(m)) => error_response(404, "not_found", &m),
            Err(CapabilityError::Refused(m)) => error_response(422, "refused", &m),
            Err(CapabilityError::Internal(m)) => error_response(500, "internal", &m),
        }
    }
}

fn json_response(status: u16, v: &Value) -> Response {
    Response::new(
        status,
        "application/json",
        serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()),
    )
}

fn error_response(status: u16, code: &str, message: &str) -> Response {
    let body = ErrorBody {
        error: ErrorDetail {
            code: code.into(),
            message: message.into(),
        },
    };
    Response::new(
        status,
        "application/json",
        serde_json::to_string_pretty(&body).unwrap_or_default(),
    )
}

/// `%XX` and `+` decoding; a malformed escape is kept as it is.
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match u8::from_str_radix(
                    std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"),
                    16,
                ) {
                    Ok(b) => {
                        out.push(b);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_parsing_and_decoding() {
        let r = Request::parse_target(
            "GET",
            "/api/v1/search?query=Git%20is%20the%20authority&limit=3&x=a%2Fb",
            vec![],
        );
        assert_eq!(r.path, "/api/v1/search");
        assert_eq!(
            r.query,
            vec![
                ("query".into(), "Git is the authority".into()),
                ("limit".into(), "3".into()),
                ("x".into(), "a/b".into())
            ]
        );
        assert_eq!(percent_decode("a+b%41%"), "a bA%");
        assert_eq!(Request::parse_target("GET", "/", vec![]).query, vec![]);
    }
}
