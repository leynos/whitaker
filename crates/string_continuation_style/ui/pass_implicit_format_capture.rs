#![deny(string_continuation_style)]

//! The pull-request-296 example must remain clean.
//!
//! A format string written directly to a format macro can capture
//! `status_line` from the surrounding scope. A format string produced by
//! `concat!()` cannot: Rust forbids implicit capture in a macro-expanded format
//! string, so the suggested rewrite would stop the program compiling. The
//! classifier routes `SourceFormatString { capture: Present }` to
//! `RequireContinuation(ImplicitFormatCapture)` and the lint stays silent.
//! Control: fail_explicit_format_arguments.rs

fn main() {
    let status_line = "HTTP/1.1 200 OK";
    let body = [0u8; 4];
    let header = format!(
        "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\n\
         Content-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
        body.len(),
    );
    assert!(header.starts_with("HTTP/1.1 200 OK"));
}
