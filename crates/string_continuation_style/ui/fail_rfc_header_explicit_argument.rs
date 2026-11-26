// run-rustfix
#![warn(string_continuation_style)]

//! RFC 0002's second acceptance fixture: the PR #296 header with an
//! explicit `status_line` argument.
//!
//! Adding `status_line = status_line` removes the implicit capture, so
//! `concat!()` no longer changes format-argument binding. The join is reported
//! and the rewrite keeps both arguments.
//!
//! Control: pass_implicit_format_capture.rs

#[path = "support.rs.inc"]
mod support;

fn header(status_line: &str, length: usize) -> String {
    format!(
        "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\n\
         Content-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
        length,
        status_line = status_line,
    )
}

fn main() {
    let header = header("200 OK", 4);
    assert!(
        support::same_bytes(
            header.as_bytes(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n"
        ),
        "the generated header must keep the RFC 0002 byte sequence: {:?}",
        header
    );
}