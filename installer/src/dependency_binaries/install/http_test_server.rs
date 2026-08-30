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
use std::sync::{Arc, Mutex, PoisonError};
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
    pub(super) fn start(routes: HashMap<String, CannedResponse>) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let requested = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = {
            let requested_for_thread = Arc::clone(&requested);
            let stop_for_thread = Arc::clone(&stop);
            thread::spawn(move || {
                run_server(&listener, &routes, &requested_for_thread, &stop_for_thread);
            })
        };
        Ok(Self {
            base_url: format!("http://127.0.0.1:{port}"),
            requested,
            stop,
            handle: Some(handle),
        })
    }

    pub(super) fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    pub(super) fn requested_paths(&self) -> Vec<String> {
        // A poisoned lock only means a test thread panicked while logging a
        // request; the recorded paths remain a valid `Vec`.
        self.requested
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take()
            && handle.join().is_err()
        {
            // The server thread panicked; tests observe failures through the
            // requests they make, so nothing further can be reported here.
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
/// Restores blocking mode and bounds reads and writes on an accepted socket.
fn configure_connection(stream: &TcpStream) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(())
}

fn serve_connection(
    mut stream: TcpStream,
    routes: &HashMap<String, CannedResponse>,
    requested: &Arc<Mutex<Vec<String>>>,
) {
    // Restore blocking mode and bound reads/writes on the accepted connection
    // (the listener is non-blocking only so the accept loop can poll for
    // shutdown). `try_clone` shares the socket, so `peer` inherits these.
    // Drop the connection if the socket cannot be configured.
    if configure_connection(&stream).is_err() {
        return;
    }
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
        let bytes_read = reader.read_line(&mut line).unwrap_or(0);
        let is_blank_line = matches!(line.as_str(), "\r\n" | "\n");
        if bytes_read == 0 || is_blank_line {
            break;
        }
    }
    // Resolve the route first — the returned references borrow `routes`, not
    // `path` — so the owned `path` can then move into the request log.
    let not_found: &[u8] = b"not found";
    let (status_line, body, declared_len) = routes.get(&path).map_or_else(
        || ("404 Not Found", not_found, not_found.len()),
        |response| {
            (
                response.status_line,
                response.body.as_slice(),
                response.declared_len,
            )
        },
    );
    // See `LocalServer::requested_paths` for why poisoning is recovered here.
    requested
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(path);
    let header = format!(
        concat!(
            "HTTP/1.1 {}\r\n",
            "Content-Length: {}\r\n",
            "Content-Type: application/octet-stream\r\n",
            "Connection: close\r\n\r\n",
        ),
        status_line, declared_len,
    );
    let write_result = stream
        .write_all(header.as_bytes())
        .and_then(|()| stream.write_all(body))
        .and_then(|()| stream.flush());
    if write_result.is_err() {
        // The client disconnected early; there is nothing further to serve.
    }
}
