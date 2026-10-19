//! Goal: a scripted HTTP/1.1 server on a local port: it answers each request with what a handler says, can break a connection mid-body, and logs every request with the (possibly paused) time it arrived.

use std::fmt::Write as _;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use reqwest::Url;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::Instant;

/// A request the server saw.
#[derive(Debug, Clone)]
pub struct Request {
    /// `GET`, `HEAD` and so on.
    pub method: String,
    /// The path and query.
    pub path: String,
    /// Header names (as sent) and values.
    pub headers: Vec<(String, String)>,
    /// When the server read it, on the runtime's clock.
    pub at: Instant,
}

impl Request {
    /// The value of a header, found without regard to case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The offset a `Range: bytes=N-` header asks for.
    pub fn range_start(&self) -> Option<usize> {
        self.header("range")?
            .strip_prefix("bytes=")?
            .strip_suffix('-')?
            .parse()
            .ok()
    }
}

/// What the server sends back.
#[derive(Debug, Clone)]
pub struct Reply {
    /// The HTTP status code.
    pub status: u16,
    /// Extra headers; the length and `Connection: close` are added.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
    /// Send only this many body bytes, announce all of them and hang up.
    pub cut_after: Option<usize>,
}

impl Reply {
    /// A `200` with a body.
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self::status(200).body(body)
    }

    /// An answer with a status and no body.
    pub fn status(code: u16) -> Self {
        Self {
            status: code,
            headers: Vec::new(),
            body: Vec::new(),
            cut_after: None,
        }
    }

    /// Sets the body.
    #[must_use]
    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    /// Adds a header.
    #[must_use]
    pub fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.to_owned(), value.into()));
        self
    }

    /// Breaks the connection after `bytes` bytes of the body.
    #[must_use]
    pub fn cut_after(mut self, bytes: usize) -> Self {
        self.cut_after = Some(bytes);
        self
    }
}

/// Answers a request for `body` the way a file server does: whole, or from the offset a `Range` names.
pub fn serve_ranges(request: &Request, body: &[u8]) -> Reply {
    let total = body.len();
    match request.range_start() {
        None => Reply::ok(body).header("Accept-Ranges", "bytes"),
        Some(start) if start >= total => {
            Reply::status(416).header("Content-Range", format!("bytes */{total}"))
        }
        Some(start) => Reply::status(206)
            .body(&body[start..])
            .header("Accept-Ranges", "bytes")
            .header(
                "Content-Range",
                format!("bytes {start}-{}/{total}", total - 1),
            ),
    }
}

type Handler = dyn Fn(&Request) -> Reply + Send + Sync;

/// A listening socket that does not answer yet, so handlers can learn their own address.
pub struct Bound {
    listener: TcpListener,
    addr: SocketAddr,
}

/// A running server; it stops when dropped.
pub struct Server {
    addr: SocketAddr,
    log: Arc<Mutex<Vec<Request>>>,
    task: tokio::task::JoinHandle<()>,
}

/// Opens a port on the loopback interface.
pub async fn bind() -> Bound {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap_or_else(|e| panic!("the test server binds: {e}"));
    let addr = listener
        .local_addr()
        .unwrap_or_else(|e| panic!("the test server has an address: {e}"));
    Bound { listener, addr }
}

impl Bound {
    /// The server's address without a path, such as `http://127.0.0.1:40123`.
    pub fn base(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Starts answering with `handler`.
    pub fn serve(self, handler: impl Fn(&Request) -> Reply + Send + Sync + 'static) -> Server {
        let handler: Arc<Handler> = Arc::new(handler);
        let log = Arc::new(Mutex::new(Vec::new()));
        let task = tokio::spawn(accept_loop(self.listener, handler, log.clone()));
        Server {
            addr: self.addr,
            log,
            task,
        }
    }
}

/// Opens a port and answers with `handler` that does not need to know its own address.
pub async fn start(handler: impl Fn(&Request) -> Reply + Send + Sync + 'static) -> Server {
    bind().await.serve(handler)
}

impl Server {
    /// The address of a path on this server.
    pub fn url(&self, path: &str) -> Url {
        Url::parse(&format!("http://{}{path}", self.addr))
            .unwrap_or_else(|e| panic!("test address parses: {e}"))
    }

    /// Every request so far, in arrival order.
    pub fn log(&self) -> Vec<Request> {
        self.log.lock().map(|log| log.clone()).unwrap_or_default()
    }

    /// The requests whose path starts with `prefix`.
    pub fn hits(&self, prefix: &str) -> Vec<Request> {
        self.log()
            .into_iter()
            .filter(|request| request.path.starts_with(prefix))
            .collect()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn accept_loop(listener: TcpListener, handler: Arc<Handler>, log: Arc<Mutex<Vec<Request>>>) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        tokio::spawn(answer(stream, handler.clone(), log.clone()));
    }
}

async fn answer(mut stream: TcpStream, handler: Arc<Handler>, log: Arc<Mutex<Vec<Request>>>) {
    let Some(request) = read_request(&mut stream).await else {
        return;
    };
    if let Ok(mut log) = log.lock() {
        log.push(request.clone());
    }
    let reply = handler(&request);
    let sends_body = request.method != "HEAD" && reply.status != 304;
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        reply.status,
        reason(reply.status),
        if sends_body { reply.body.len() } else { 0 }
    );
    for (name, value) in &reply.headers {
        let _ = write!(head, "{name}: {value}\r\n");
    }
    head.push_str("\r\n");
    if stream.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    if sends_body {
        let sent = reply
            .cut_after
            .unwrap_or(reply.body.len())
            .min(reply.body.len());
        if stream.write_all(&reply.body[..sent]).await.is_err() {
            return;
        }
    }
    let _ = stream.shutdown().await;
}

async fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut head = Vec::new();
    let mut chunk = [0u8; 1024];
    while !head.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 || head.len() > 64 * 1024 {
            return None;
        }
        head.extend_from_slice(&chunk[..read]);
    }
    let text = String::from_utf8_lossy(&head).into_owned();
    let mut lines = text.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_owned();
    let path = first.next()?.to_owned();
    let headers = lines
        .take_while(|line| !line.is_empty())
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
        .collect();
    Some(Request {
        method,
        path,
        headers,
        at: Instant::now(),
    })
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        206 => "Partial Content",
        304 => "Not Modified",
        404 => "Not Found",
        416 => "Range Not Satisfiable",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}
