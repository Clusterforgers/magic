//! A local stand-in for the Scryfall CDN: it answers every path with a small picture that names the path, counts the requests and can be told to fail, stall or lie.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::header::{ACCEPT, RETRY_AFTER, USER_AGENT};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// What the CDN does with the next requests.
#[derive(Debug, Clone)]
pub enum Behavior {
    /// Answers 200 with the picture.
    Serve,
    /// Answers this status every time.
    Fail(u16),
    /// Answers `status` (with `Retry-After` when given) to `remaining` requests, then serves.
    FailFirst {
        remaining: usize,
        status: u16,
        retry_after: Option<u64>,
    },
    /// Answers 200 with a web page instead of a picture.
    Page,
    /// Waits, then serves.
    Delay(Duration),
}

/// One request the CDN saw.
#[derive(Debug, Clone)]
pub struct Hit {
    pub path: String,
    pub user_agent: Option<String>,
    pub accept: Option<String>,
}

struct Shared {
    hits: Mutex<Vec<Hit>>,
    behavior: Mutex<Behavior>,
}

/// The running fake CDN; it stops with the test.
pub struct Cdn {
    pub addr: SocketAddr,
    shared: Arc<Shared>,
    task: JoinHandle<()>,
}

/// The bytes the CDN answers for `path`: the format's magic number, then the path.
pub fn body_for(path: &str) -> Vec<u8> {
    let mut body = if path.contains(".png") {
        vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
    } else {
        vec![0xFF, 0xD8, 0xFF, 0xE0]
    };
    body.extend_from_slice(path.as_bytes());
    body
}

impl Cdn {
    /// Binds a free port on localhost and serves there.
    pub async fn start() -> Cdn {
        let shared = Arc::new(Shared {
            hits: Mutex::new(Vec::new()),
            behavior: Mutex::new(Behavior::Serve),
        });
        let app = Router::new().fallback(answer).with_state(shared.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Cdn { addr, shared, task }
    }

    /// `http://127.0.0.1:port`, what the server is told to ask instead of the stored host.
    pub fn base(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Changes what the CDN does from now on.
    pub fn set(&self, behavior: Behavior) {
        *self.shared.behavior.lock().unwrap() = behavior;
    }

    /// Every request so far.
    pub fn hits(&self) -> Vec<Hit> {
        self.shared.hits.lock().unwrap().clone()
    }

    /// How many requests there were.
    pub fn count(&self) -> usize {
        self.shared.hits.lock().unwrap().len()
    }

    /// The paths requested, in order.
    pub fn paths(&self) -> Vec<String> {
        self.hits().into_iter().map(|hit| hit.path).collect()
    }
}

impl Drop for Cdn {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn answer(State(shared): State<Arc<Shared>>, uri: Uri, headers: HeaderMap) -> Response {
    let text = |name| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    let path = match uri.query() {
        Some(query) => format!("{}?{query}", uri.path()),
        None => uri.path().to_owned(),
    };
    shared.hits.lock().unwrap().push(Hit {
        path: path.clone(),
        user_agent: text(USER_AGENT),
        accept: text(ACCEPT),
    });
    let behavior = {
        let mut behavior = shared.behavior.lock().unwrap();
        if let Behavior::FailFirst {
            remaining,
            status,
            retry_after,
        } = &mut *behavior
        {
            if *remaining > 0 {
                *remaining -= 1;
                let mut reply = StatusCode::from_u16(*status).unwrap().into_response();
                if let Some(seconds) = retry_after {
                    reply
                        .headers_mut()
                        .insert(RETRY_AFTER, seconds.to_string().parse().unwrap());
                }
                return reply;
            }
            Behavior::Serve
        } else {
            behavior.clone()
        }
    };
    match behavior {
        Behavior::Fail(status) => StatusCode::from_u16(status).unwrap().into_response(),
        Behavior::Page => "<html>not a picture</html>".into_response(),
        Behavior::Delay(wait) => {
            tokio::time::sleep(wait).await;
            body_for(&path).into_response()
        }
        Behavior::Serve | Behavior::FailFirst { .. } => body_for(&path).into_response(),
    }
}
