//! Routing and binding, transport-neutral: a request in, a response out.
//!
//! The router names no surface. It asks [`super::surfaces::Served`] — the web topology this
//! process resolved, narrowed to what it can answer — who owns a path, and dispatches to
//! what that surface bound. A route under `/api/v1/` is a capability with an HTTP exposure;
//! `/`, `/openapi.json`, `/swagger`, `/cockpit` and `/mcp` are routes the executable answers
//! itself; `/docs/` and every generated report are directories a producer wrote. All of them
//! are declared once, in `crate::web::discover`, and reach the router, the home page, the
//! `web.surfaces` capability and the publication from there.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::capability::{CapabilityError, CaseContext, Context, HttpMethod};
use crate::cockpit::Cockpit;
use crate::web::discover::Runtime;
use crate::web::home;

use super::events;
use super::mcp::McpEndpoint;
use super::surfaces::{Bound, Native, Served};
use super::swagger::Swagger;
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

    /// The request that carries `input` to a capability's route the way the binding says:
    /// `GET` puts every top-level property that is not `null` on the query string (a
    /// scalar as its text, anything else as JSON), `POST` sends the input as the body.
    /// The inverse of the router's binding, so a benchmark case reaches the route as a
    /// client would send it.
    ///
    /// ```
    /// use majordomus_cli::capability::HttpMethod;
    /// use majordomus_cli::http::Request;
    /// use serde_json::json;
    /// let r = Request::bind(HttpMethod::Get, "/api/v1/search", &json!({ "query": "a b", "limit": 2, "kind": null }));
    /// assert_eq!(r.path, "/api/v1/search");
    /// assert_eq!(r.query, vec![("query".to_string(), "a b".to_string()), ("limit".to_string(), "2".to_string())]);
    /// let p = Request::bind(HttpMethod::Post, "/api/v1/peers/announce", &json!({ "intent": "x" }));
    /// assert_eq!(p.body, br#"{"intent":"x"}"#);
    /// ```
    pub fn bind(method: HttpMethod, path: &str, input: &Value) -> Self {
        match method {
            HttpMethod::Get => Request {
                method: "GET".into(),
                path: path.into(),
                query: input
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .filter(|(_, v)| !v.is_null())
                            .map(|(k, v)| {
                                let text = match v {
                                    Value::String(s) => s.clone(),
                                    other => other.to_string(),
                                };
                                (k.clone(), text)
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                headers: Vec::new(),
                body: Vec::new(),
            },
            HttpMethod::Post => Request {
                method: "POST".into(),
                path: path.into(),
                query: Vec::new(),
                headers: Vec::new(),
                body: input.to_string().into_bytes(),
            },
        }
    }

    /// The request target this request would be sent as: the path with the query string,
    /// percent-encoded.
    ///
    /// ```
    /// use majordomus_cli::capability::HttpMethod;
    /// use majordomus_cli::http::Request;
    /// let r = Request::bind(HttpMethod::Get, "/api/v1/search", &serde_json::json!({ "query": "a b&c" }));
    /// assert_eq!(r.target(), "/api/v1/search?query=a%20b%26c");
    /// ```
    pub fn target(&self) -> String {
        if self.query.is_empty() {
            return self.path.clone();
        }
        let pairs: Vec<String> = self
            .query
            .iter()
            .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
            .collect();
        format!("{}?{}", self.path, pairs.join("&"))
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

/// What a response carries. Text for everything this projection computes; bytes for a file
/// it serves from disk, which is not always UTF-8 and must not be repaired into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// A string this process produced.
    Text(String),
    /// Bytes read from somewhere else, passed through unchanged.
    Bytes(Vec<u8>),
}

impl Body {
    /// The bytes on the wire.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Body::Text(t) => t.as_bytes(),
            Body::Bytes(b) => b,
        }
    }

    /// How many bytes it is.
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }

    /// Is it empty?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The body as text: borrowed when it is text, and lossy when it is not, which only a
    /// diagnostic or a test ever asks for.
    ///
    /// ```
    /// use majordomus_cli::http::router::Body;
    /// assert_eq!(Body::Text("a".into()).text(), "a");
    /// assert_eq!(Body::Bytes(vec![0xff]).text(), "\u{fffd}");
    /// ```
    pub fn text(&self) -> std::borrow::Cow<'_, str> {
        match self {
            Body::Text(t) => std::borrow::Cow::Borrowed(t),
            Body::Bytes(b) => String::from_utf8_lossy(b),
        }
    }
}

impl From<String> for Body {
    fn from(value: String) -> Self {
        Body::Text(value)
    }
}

impl From<&str> for Body {
    fn from(value: &str) -> Self {
        Body::Text(value.to_string())
    }
}

impl From<Vec<u8>> for Body {
    fn from(value: Vec<u8>) -> Self {
        Body::Bytes(value)
    }
}

impl PartialEq<str> for Body {
    fn eq(&self, other: &str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl PartialEq<&str> for Body {
    fn eq(&self, other: &&str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl std::fmt::Display for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text())
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
    pub body: Body,
    /// Further headers (`Mcp-Session-Id`); the server adds `Content-Length` and `Cache-Control`.
    pub headers: Vec<(String, String)>,
}

impl Response {
    /// A response with no further headers.
    pub fn new(status: u16, content_type: &'static str, body: impl Into<Body>) -> Self {
        Response {
            status,
            content_type,
            body: body.into(),
            headers: Vec::new(),
        }
    }

    /// The same response with one more header.
    pub fn with_header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.to_string(), value.into()));
        self
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
    /// What kind of failure it is: `invalid_input`, `not_found`, `method_not_allowed`,
    /// `refused`, `forbidden`, `too_large`, `unavailable` (a surface whose producer has not
    /// run) or `internal`.
    pub code: String,
    /// The reason, for a person.
    pub message: String,
}

#[derive(Clone)]
/// Routes requests to the surfaces this process serves. Cheap to clone: every worker
/// thread holds one, and the resolution behind it is shared.
pub struct Router {
    ctx: Arc<Context>,
    version: &'static str,
    /// The OpenAPI document, rendered once: the registry is immutable for the process.
    openapi: Arc<std::sync::OnceLock<Result<String, String>>>,
    /// The Swagger UI shell, rendered once for the same reason and naming this checkout.
    swagger: Arc<std::sync::OnceLock<String>>,
    /// MCP over HTTP at the mount its surface declares, when this router serves a shared
    /// server.
    mcp: Option<Arc<McpEndpoint>>,
    /// The Cockpit, when the process located a distribution to serve its assets from.
    cockpit: Option<Arc<Cockpit>>,
    /// The API viewer, with the files it loads read from the same distribution. Unlike the
    /// Cockpit it is never absent: without a distribution it renders a page that says the
    /// viewer is not in this one, which is a great deal more use than a blank frame.
    swagger: Arc<Swagger>,
    /// The resolved surfaces and their handlers, and the context narrowed to them. Built
    /// on first use, because the builder learns what this process offers after `new`.
    served: Arc<std::sync::OnceLock<Result<Resolution, String>>>,
    /// Which git repository the served checkout belongs to, asked once: the index route
    /// answers it on every probe, and a probe must not cost a subprocess.
    git: Arc<std::sync::OnceLock<Option<crate::repository::GitIdentity>>>,
}

/// What one router serves: the bound surfaces, and the context every capability call is
/// given so that `web.surfaces` answers with what this process actually serves.
struct Resolution {
    served: Served,
    ctx: Arc<Context>,
}

impl Router {
    /// A router over a loaded context, without `/mcp` and without the Cockpit.
    pub fn new(ctx: Arc<Context>, version: &'static str) -> Self {
        crate::perf::Counters::bump(&crate::perf::COUNTERS.http_projection_builds);
        Router {
            ctx,
            version,
            openapi: Arc::new(std::sync::OnceLock::new()),
            swagger: Arc::new(std::sync::OnceLock::new()),
            mcp: None,
            cockpit: None,
            swagger: Arc::new(Swagger::new(None)),
            served: Arc::new(std::sync::OnceLock::new()),
            git: Arc::new(std::sync::OnceLock::new()),
        }
    }

    /// The same router, serving the Cockpit's pages — and the API viewer's own files —
    /// from `share_dir`. One distribution, and the two surfaces that read static files out
    /// of it: giving them one builder is what stops a process from having the Cockpit's
    /// stylesheet and not the viewer's.
    pub fn with_cockpit(mut self, share_dir: Option<&std::path::Path>) -> Self {
        self.cockpit = Some(Arc::new(Cockpit::new(
            Arc::clone(&self.ctx),
            self.version,
            share_dir,
        )));
        self.swagger = Arc::new(Swagger::new(share_dir));
        self
    }

    /// The same router, serving MCP over HTTP through `endpoint`.
    pub fn with_mcp(mut self, endpoint: Arc<McpEndpoint>) -> Self {
        self.mcp = Some(endpoint);
        self
    }

    /// What this process offers, as the topology's feature gates read it.
    fn runtime(&self) -> Runtime {
        Runtime {
            mcp: self.mcp.is_some(),
            cockpit: self.cockpit.is_some(),
        }
    }

    /// The resolved surfaces, narrowed and validated once.
    fn resolution(&self) -> Result<&Resolution, &String> {
        self.served
            .get_or_init(|| {
                let root = std::path::Path::new(&self.ctx.index.repository.root);
                Served::resolve(&self.ctx.web, root, self.runtime())
                    .map(|served| Resolution {
                        ctx: Arc::new(self.ctx.with_web(served.shared())),
                        served,
                    })
                    // the reason alone: `served()` puts it back into the same error, and a
                    // message that names its own kind twice reads as a bug in the tool
                    .map_err(|e| match e {
                        crate::error::Error::InvalidSurface { reason, .. } => reason,
                        other => other.to_string(),
                    })
            })
            .as_ref()
    }

    /// The surfaces this router serves, for a caller that wants to describe them: the
    /// server's startup line, and the tests.
    ///
    /// The answer is the resolution, not a fresh discovery — a router that could not
    /// resolve its topology never answered a request either, and says the same thing here
    /// that it says to a client.
    pub fn served(&self) -> crate::error::Result<&Served> {
        match self.resolution() {
            Ok(resolution) => Ok(&resolution.served),
            Err(reason) => Err(crate::error::Error::InvalidSurface {
                surface: "topology".into(),
                reason: reason.clone(),
            }),
        }
    }

    /// Does this request open the live channel, and may it?
    ///
    /// The server asks before it reads a body or writes a response, because an upgrade is
    /// the one thing a router cannot answer: it hands the socket over. `None` means the
    /// request is not for the live channel at all and is routed normally.
    pub fn websocket(&self, req: &Request) -> Option<Result<events::Accepted, Response>> {
        if req.path != events::PATH {
            return None;
        }
        if !super::ws::is_upgrade(req) {
            return None;
        }
        // a process whose topology does not resolve serves nothing, the live channel
        // included: the same answer every other path gets
        if let Err(reason) = self.resolution() {
            return Some(Err(Response::error(500, "internal", reason)));
        }
        Some(events::accept(self.ctx.executions.store(), req))
    }

    fn openapi(&self) -> Response {
        let rendered = self.openapi.get_or_init(|| {
            let cases = CaseContext {
                index: &self.ctx.index,
            };
            openapi::document(&self.ctx.registry, self.version, Some(&cases))
                .map(|d| openapi::render(&d))
        });
        match rendered {
            Ok(text) => Response::new(200, "application/json", text.clone()),
            Err(e) => error_response(500, "internal", e),
        }
    }

    /// Answer one request: the surface that owns its path, or nothing.
    ///
    /// A request that carries an `Origin` header came from a page in a browser. A browser
    /// cannot read a cross-origin response without the headers this server never sends,
    /// so a read is already contained; a request that changes something is not, and one
    /// from another origin is refused before it reaches a handler. A client that is not a
    /// browser sends no `Origin` and is unaffected.
    pub fn handle(&self, req: &Request) -> Response {
        if req.method != "GET" && req.method != "HEAD" {
            if let Some(refusal) = self.foreign_origin(req) {
                return refusal;
            }
        }
        let resolution = match self.resolution() {
            Ok(resolution) => resolution,
            Err(reason) => return error_response(500, "internal", reason),
        };
        let Some((surface, bound)) = resolution.served.owner(&req.path) else {
            return error_response(
                404,
                "not_found",
                &format!(
                    "no surface owns {}; what this process serves is listed at / and at {}web/surfaces",
                    req.path,
                    crate::capability::model::HttpExposure::PREFIX
                ),
            );
        };
        // a surface owns its mount and everything under it, which is what a directory and a
        // prefix want and what a single route does not: `/swagger/anything` is not the
        // Swagger UI, and answering it as though it were would invent a route nothing
        // declared. The surfaces that answer one path say so here, once.
        //
        // The single exception is declared, not incidental: the API viewer's own files at
        // `/swagger/assets/`, which are as much part of that page as the Cockpit's
        // stylesheet is of its own. Everything else under `/swagger` is still a 404.
        let exact = req.path == surface.mount.as_str() || req.path == surface.mount.prefix();
        let viewer_asset = matches!(bound, Bound::Route(Native::Swagger))
            && req.path.starts_with(swagger::ASSET_PREFIX);
        // the home page is not among them: its mount is the root, which owns every path
        // nothing else claims, and it answers those with a 404 that names what is served
        let single_path = matches!(
            bound,
            Bound::Route(Native::OpenApi | Native::Swagger | Native::Mcp | Native::Events)
        );
        if single_path && !exact && !viewer_asset {
            return error_response(
                404,
                "not_found",
                &format!(
                    "{} is the whole of the '{}' surface; nothing is served under it. What this process serves is listed at / and at {}web/surfaces",
                    surface.mount,
                    surface.id,
                    crate::capability::model::HttpExposure::PREFIX
                ),
            );
        }
        match bound {
            Bound::Directory(files) => {
                if req.method != "GET" {
                    return error_response(
                        405,
                        "method_not_allowed",
                        &format!(
                            "{} is a generated directory; it is read with GET",
                            surface.mount
                        ),
                    );
                }
                files.respond(&req.path)
            }
            Bound::Route(Native::Home) => self.home(req, resolution),
            Bound::Route(Native::OpenApi) => self.openapi(),
            Bound::Route(Native::Swagger) => swagger_response(self),
            Bound::Route(Native::Mcp) => match &self.mcp {
                Some(endpoint) => endpoint.handle(req),
                None => error_response(
                    500,
                    "internal",
                    "the MCP surface is served with no endpoint behind it",
                ),
            },
            // the upgrade itself never reaches here: the server answers it before routing,
            // because handing the socket over is something only the server can do. What
            // reaches here is a request that did not ask to be upgraded, and the reply says
            // what this path is and where the same events are readable over HTTP.
            Bound::Route(Native::Events) => match events::accept(self.ctx.executions.store(), req) {
                Ok(_) => Response::error(
                    500,
                    "internal",
                    "a WebSocket upgrade reached the router; the server answers it before routing",
                ),
                Err(response) => response,
            },
            Bound::Route(Native::Cockpit) => match &self.cockpit {
                Some(cockpit) => cockpit.handle(req),
                None => error_response(
                    500,
                    "internal",
                    "the Cockpit surface is served with no Cockpit behind it",
                ),
            },
            Bound::Route(Native::Api) => {
                let prefix = crate::capability::model::HttpExposure::PREFIX;
                if req.path == prefix || req.path == prefix.trim_end_matches('/') {
                    return self.routes(&resolution.ctx);
                }
                self.capability(req, &resolution.ctx)
            }
        }
    }

    /// `/`: the home page for a browser, the same topology as JSON for everything else.
    ///
    /// The root surface owns every path nothing else claims, which is how a request for a
    /// path no surface declares reaches the most useful 404 this server can give: one from
    /// a process that knows what it does serve.
    fn home(&self, req: &Request, resolution: &Resolution) -> Response {
        if req.path != "/" {
            return error_response(
                404,
                "not_found",
                &format!(
                    "no surface owns {}; what this process serves is listed at / and at {}web/surfaces",
                    req.path,
                    crate::capability::model::HttpExposure::PREFIX
                ),
            );
        }
        if req.method != "GET" {
            return error_response(405, "method_not_allowed", "the home page is read with GET");
        }
        let topology = resolution.served.topology();
        if prefers_html(req) {
            let id = self.repository_id();
            let identity = self.identity(&id);
            let ready = |id: &str| {
                if resolution.served.ready(id) {
                    home::Availability::Ready
                } else {
                    home::Availability::NotBuilt
                }
            };
            return Response::new(
                200,
                "text/html; charset=utf-8",
                home::page(topology, &identity, &ready),
            );
        }
        let surfaces: Vec<Value> = topology
            .surfaces
            .iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "title": s.title,
                    "path": s.mount.as_str(),
                    "category": s.category.to_string(),
                    "visibility": s.visibility.to_string(),
                    "kind": s.kind.to_string(),
                    "ready": resolution.served.ready(&s.id),
                })
            })
            .collect();
        let git = self.git.get_or_init(|| {
            crate::repository::git_identity(std::path::Path::new(&self.ctx.index.repository.root))
        });
        // whether this process is still the checkout's server. A server whose lease was
        // taken over keeps serving the peers it already has, so it goes on answering here,
        // and every other field is as true of it as of the current server. This is the one
        // that tells them apart; `lease::probe` reads it under the same name, which is why
        // the name is the constant and not a literal.
        let mut index = json!({
                "name": "majordomus",
                "version": self.version,
                "description": crate::about::SUMMARY,
                "reference": crate::about::REFERENCE_URL,
                // the repository's name and not its path: this answer is served to whoever
                // can reach the socket, and where the checkout sits on the host is of no
                // use to them and of some use to somebody else. The HTML page has always
                // withheld it; the two projections of one surface now agree.
                "repository": repository_name(&self.ctx.index.repository.root),
                // which repository, without saying where it is: a second process that
                // already knows the root computes the same value and knows this server is
                // its own (crate::repository::identity)
                "repository_id": crate::repository::identity(
                    std::path::Path::new(&self.ctx.index.repository.root),
                ),
                // and which git repository that checkout belongs to: one value for every
                // worktree of it, so that a reader can tell two servers of one repository
                // from the servers of two (crate::repository::git_identity)
                "git_repository_id": git.as_ref().map(|g| g.id.clone()),
                "linked_worktree": git.as_ref().is_some_and(|g| g.linked),
                "surfaces": surfaces,
        });
        index[crate::lease::LEASEHOLDER_KEY] = json!(!crate::lease::was_lost());
        json_response(200, &index)
    }

    /// The refusal for a state-changing request from another origin, when there is one.
    /// The allowed origins are this server's own: whatever host the request was addressed
    /// to. Nothing else is configured, because nothing else should be able to change this
    /// process from a browser.
    fn foreign_origin(&self, req: &Request) -> Option<Response> {
        let origin = req.header("origin")?;
        // `null` is what a sandboxed frame or a `file://` page sends; neither is this server
        let host = req.header("host").unwrap_or_default();
        let allowed = [format!("http://{host}"), format!("https://{host}")];
        if allowed.iter().any(|a| a == origin) {
            return None;
        }
        tracing::warn!(
            origin = origin,
            method = %req.method,
            path = %req.path,
            "a state-changing request from another origin was refused"
        );
        Some(error_response(
            403,
            "forbidden",
            &format!(
                "a {} from origin '{origin}' is refused: this server accepts a state-changing request from a browser only from its own origin",
                req.method
            ),
        ))
    }

    /// The capability mount itself: every route under it, from the registry that declares
    /// them.
    ///
    /// It exists so that the surface the home page offers is a page a person can open
    /// rather than a prefix that answers 404, and it is the registry's own list rather
    /// than a second one — the schemas, the parameters and the examples stay in the
    /// OpenAPI document, which is generated from the same descriptors.
    fn routes(&self, ctx: &Arc<Context>) -> Response {
        // The registry's own list, in the registry's own order — the same order the
        // Cockpit's table and the site's route view read in, because all three ask it the
        // same question. The sort that used to stand here compared `Value::to_string()`,
        // which is the JSON encoding of the path, quotes included, and had no method
        // tiebreak.
        let routes: Vec<Value> = ctx
            .registry
            .http_routes()
            .into_iter()
            .map(|r| {
                json!({
                    "method": r.http.method.as_str(),
                    "path": r.http.path,
                    "capability": r.capability.id.as_str(),
                    "title": r.capability.title,
                })
            })
            .collect();
        json_response(
            200,
            &json!({
                "description": "Every capability with an HTTP exposure, as the registry declares it.",
                "openapi": swagger::SPEC_PATH,
                "swagger": swagger::SWAGGER_PATH,
                "routes": routes,
            }),
        )
    }

    fn capability(&self, req: &Request, ctx: &Arc<Context>) -> Response {
        let Some(method) = HttpMethod::parse(&req.method) else {
            return error_response(
                405,
                "method_not_allowed",
                &format!("method {} is not served", req.method),
            );
        };
        let Some(c) = ctx.registry.by_http(method, &req.path) else {
            let other_method = [HttpMethod::Get, HttpMethod::Post]
                .into_iter()
                .filter(|m| *m != method)
                .any(|m| ctx.registry.by_http(m, &req.path).is_some());
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
        match ctx.execute(c.id.as_str(), input) {
            Ok(v) => json_response(200, &v),
            Err(CapabilityError::InvalidInput(m)) => error_response(400, "invalid_input", &m),
            Err(CapabilityError::NotFound(m)) => error_response(404, "not_found", &m),
            Err(CapabilityError::Refused(m)) => error_response(422, "refused", &m),
            Err(CapabilityError::Internal(m)) => error_response(500, "internal", &m),
        }
    }
}

/// Does this request come from something that would rather have a page than a document?
/// Only an explicit `text/html` in `Accept` counts: a client that sends none, or `*/*`,
/// gets the JSON index, which is what every existing caller does.
///
/// ```
/// use majordomus_cli::http::{router::prefers_html, Request};
/// let plain = Request::parse_target("GET", "/", vec![]);
/// assert!(!prefers_html(&plain));
/// let browser = plain.clone().with_headers(vec![
///     ("Accept".into(), "text/html,application/xhtml+xml,*/*;q=0.8".into()),
/// ]);
/// assert!(prefers_html(&browser));
/// ```
pub fn prefers_html(req: &Request) -> bool {
    req.header("accept").is_some_and(|accept| {
        accept.split(',').any(|part| {
            part.split(';')
                .next()
                .is_some_and(|t| t.trim() == "text/html")
        })
    })
}

/// The repository's name: the last component of its root, which is what a person calls it.
/// The root itself is a filesystem path and is not put on a page anyone can reach.
impl Router {
    /// The identity of the repository this process answers for: the root hashed, never
    /// the root. Computed here so that every surface naming a checkout names the same one.
    fn repository_id(&self) -> String {
        crate::repository::identity(std::path::Path::new(&self.ctx.index.repository.root))
    }

    /// What every surface says about the process, from the model this router already
    /// holds rather than from a second look at the filesystem.
    fn identity<'a>(&'a self, id: &'a str) -> home::Identity<'a> {
        home::Identity {
            version: self.version,
            summary: crate::about::SUMMARY,
            repository: repository_name(&self.ctx.index.repository.root),
            id,
            revision: match &self.ctx.index.repository.git {
                crate::git::GitState::Available(info) => info.head.as_deref(),
                crate::git::GitState::Unavailable { .. } => None,
            },
            capabilities: self.ctx.registry.summary().total,
        }
    }

    /// The Swagger UI shell for this process, rendered once: the registry and the
    /// repository are immutable for its lifetime, so the banner cannot go stale within it.
    fn swagger_page(&self) -> &str {
        self.swagger.get_or_init(|| {
            let id = self.repository_id();
            swagger::page(&self.identity(&id))
        })
    }
}

fn repository_name(root: &str) -> &str {
    root.rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or("repository")
}

/// The Swagger UI shell. `/swagger` and `/swagger/` both answer it: the page loads its
/// distribution and the document by absolute path, so neither form can resolve wrongly.
fn swagger_response(router: &Router) -> Response {
    Response::new(
        200,
        "text/html; charset=utf-8",
        router.swagger_page().to_string(),
    )
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

/// Percent-encode everything but the unreserved characters of RFC 3986.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
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
    fn a_body_is_text_or_bytes_and_says_so_the_same_way() {
        let text = Body::from("hello".to_string());
        assert_eq!(text.len(), 5);
        assert!(!text.is_empty());
        assert_eq!(text.as_bytes(), b"hello");
        assert_eq!(text, *"hello");
        assert_eq!(format!("{text}"), "hello");
        assert!(matches!(text.text(), std::borrow::Cow::Borrowed(_)));

        let bytes = Body::from(vec![0x89, b'P', b'N', b'G']);
        assert_eq!(bytes.len(), 4);
        assert_eq!(bytes.as_bytes()[0], 0x89);
        assert_ne!(bytes, *"PNG");
        // a file that is not UTF-8 is passed through unchanged and only repaired when a
        // diagnostic asks for text
        assert!(matches!(bytes.text(), std::borrow::Cow::Owned(_)));
        assert!(Body::from(Vec::new()).is_empty());
        assert_eq!(Body::from("a"), Body::Text("a".into()));
    }

    #[test]
    fn a_response_carries_the_headers_it_was_given() {
        let r = Response::new(200, "text/plain; charset=utf-8", "hi".to_string())
            .with_header("Cache-Control", "no-cache");
        assert_eq!(r.status, 200);
        assert_eq!(r.body, *"hi");
        assert!(r
            .headers
            .iter()
            .any(|(k, v)| k == "Cache-Control" && v == "no-cache"));

        let e = Response::error(404, "not_found", "nothing here");
        assert_eq!(e.status, 404);
        assert_eq!(e.content_type, "application/json");
        let parsed: ErrorBody = serde_json::from_str(&e.body.text()).expect("errors are JSON");
        assert_eq!(parsed.error.code, "not_found");
        assert_eq!(parsed.error.message, "nothing here");
    }

    #[test]
    fn the_repository_is_named_and_its_path_is_not() {
        assert_eq!(
            repository_name("/a/b/prismatic-majordomus"),
            "prismatic-majordomus"
        );
        assert_eq!(repository_name("/a/b/repo/"), "repo");
        assert_eq!(repository_name("/"), "repository");
        assert_eq!(repository_name(""), "repository");
    }

    #[test]
    fn the_swagger_shell_is_the_same_page_however_it_is_reached() {
        let page = swagger_response(self);
        assert_eq!(page.status, 200);
        assert!(page.content_type.starts_with("text/html"));
        assert!(page.body.text().contains("/openapi.json"));
    }

    #[test]
    fn only_an_explicit_html_accept_asks_for_a_page() {
        let plain = Request::parse_target("GET", "/", vec![]);
        assert!(!prefers_html(&plain));
        for accept in [
            "text/html",
            "text/html,application/xhtml+xml",
            "application/json, text/html;q=0.9",
        ] {
            let r = plain
                .clone()
                .with_headers(vec![("Accept".into(), accept.into())]);
            assert!(prefers_html(&r), "{accept}");
        }
        for accept in ["*/*", "application/json", "text/plain"] {
            let r = plain
                .clone()
                .with_headers(vec![("Accept".into(), accept.into())]);
            assert!(!prefers_html(&r), "{accept}");
        }
    }

    #[test]
    fn a_request_binds_the_way_the_projection_says_and_reads_back_the_same() {
        let get = Request::bind(
            HttpMethod::Get,
            "/api/v1/search",
            &json!({ "query": "a b", "limit": 2, "kind": null }),
        );
        assert_eq!(get.target(), "/api/v1/search?query=a%20b&limit=2");
        let back = Request::parse_target("GET", &get.target(), vec![]);
        assert_eq!(back.query, get.query);
        assert_eq!(back.path, get.path);

        let post = Request::bind(HttpMethod::Post, "/api/v1/x", &json!({ "a": 1 }));
        assert_eq!(post.target(), "/api/v1/x");
        assert_eq!(post.body, br#"{"a":1}"#);
    }

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
