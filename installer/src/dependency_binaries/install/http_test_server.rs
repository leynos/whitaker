//! A loopback-only HTTP/1.1 test server for driving the download workflow.
//!
//! Shared test support for [`super::downloader_boundary_tests`]: it answers a
//! fixed route table, records requested paths, and shuts down cleanly on drop.
//! Kept in its own module so the boundary-test file stays within its size
//! budget.

use std::collections::HashMap;
use std::io;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// One canned HTTP/1.1 response body served for a matched path. `declared_len`
/// is the advertised `Content-Length`, which normally matches `body`.
pub(super) struct CannedResponse {
    status_line: &'static str,
    body: Vec<u8>,
    declared_len: usize,
}

impl CannedResponse {
    pub(super) fn ok(body: Vec<u8>) -> Self {
        Self {
            status_line: "200 OK",
            declared_len: body.len(),
            body,
        }
    }

    /// Advertise `declared_len` bytes but send only `body` before closing, so
    /// the client fails part-way through reading the response body.
    pub(super) fn truncated(body: Vec<u8>, declared_len: usize) -> Self {
        Self {
            status_line: "200 OK",
            body,
            declared_len,
        }
    }
}

/// A loopback-only HTTP/1.1 server for the download workflow. It answers a fixed
/// route table, records requested paths, and shuts down cleanly on drop.
pub(super) struct LocalServer {
    base_url: String,
    requested: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl LocalServer {
    pub(super) fn start(routes: HashMap<String, CannedResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback listener");
        let port = listener.local_addr().expect("resolve local addr").port();
        listener
            .set_nonblocking(true)
            .expect("set listener non-blocking");
        let requested = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = {
            let requested = Arc::clone(&requested);
            let stop = Arc::clone(&stop);
            thread::spawn(move || run_server(&listener, &routes, &requested, &stop))
        };
        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            requested,
            stop,
            handle: Some(handle),
        }
    }

    pub(super) fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    pub(super) fn requested_paths(&self) -> Vec<String> {
        self.requested.lock().expect("lock requested paths").clone()
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Accept connections until `stop` is set, serving each through the route table.
/// Non-blocking polling keeps the loop responsive to shutdown when idle.
fn run_server(
    listener: &TcpListener,
    routes: &HashMap<String, CannedResponse>,
    requested: &Arc<Mutex<Vec<String>>>,
    stop: &AtomicBool,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => serve_connection(stream, routes, requested),
            Err(ref error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => break,
        }
    }
}

/// Read one request, record its path, and write the matching canned response
/// (or a 404). `Connection: close` lets the client frame the response end.
fn serve_connection(
    mut stream: TcpStream,
    routes: &HashMap<String, CannedResponse>,
    requested: &Arc<Mutex<Vec<String>>>,
) {
    // Restore blocking mode and bound reads/writes on the accepted connection
    // (the listener is non-blocking only so the accept loop can poll for
    // shutdown). `try_clone` shares the socket, so `peer` inherits these.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let Ok(peer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(peer);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    // Drain the remaining request headers up to the blank line.
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) if line == "\r\n" || line == "\n" => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    // Resolve the route first — the returned references borrow `routes`, not
    // `path` — so the owned `path` can then move into the request log.
    let (status_line, body, declared_len): (&str, &[u8], usize) = match routes.get(&path) {
        Some(response) => (response.status_line, &response.body, response.declared_len),
        None => ("404 Not Found", b"not found", b"not found".len()),
    };
    requested.lock().expect("lock requested paths").push(path);
    let header = format!(
        concat!(
            "HTTP/1.1 {}\r\n",
            "Content-Length: {}\r\n",
            "Content-Type: application/octet-stream\r\n",
            "Connection: close\r\n\r\n",
        ),
        status_line, declared_len,
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}
