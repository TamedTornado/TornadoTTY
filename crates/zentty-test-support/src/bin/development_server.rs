#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

// This is only a dead-fixture ceiling. The owning product journey terminates
// the process as soon as its assertions finish. Keep enough headroom for the
// same real journey to run under the qualification matrix's concurrent load.
const LIFETIME: Duration = Duration::from_secs(300);

fn main() -> Result<(), String> {
    let receipt = receipt_argument()?;
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("could not bind development server: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("could not bound development server polling: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("could not inspect development server address: {error}"))?;
    let url = format!("http://{address}/fixture");
    std::fs::write(&receipt, format!("url={url}\npid={}\n", std::process::id()))
        .map_err(|error| format!("could not publish development server receipt: {error}"))?;
    println!("Development server: {url}");
    std::io::stdout()
        .flush()
        .map_err(|error| format!("could not publish development server URL: {error}"))?;

    let deadline = Instant::now() + LIFETIME;
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((stream, _)) => respond(stream)?,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(format!("development server accept failed: {error}")),
        }
    }
    Err("development server fixture exceeded its bounded lifetime".into())
}

fn receipt_argument() -> Result<PathBuf, String> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--receipt")) {
        return Err("usage: development_server --receipt ABSOLUTE_PATH".into());
    }
    let path = arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "--receipt requires a path".to_owned())?;
    if arguments.next().is_some() || !path.is_absolute() || path.exists() {
        return Err("receipt must be one absent absolute path".into());
    }
    Ok(path)
}

fn respond(mut stream: TcpStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("could not bound development server read: {error}"))?;
    let mut request = [0_u8; 4096];
    let count = stream
        .read(&mut request)
        .map_err(|error| format!("development server read failed: {error}"))?;
    if count == 0 || !request[..count].starts_with(b"GET ") {
        return Err("development server received a non-GET request".into());
    }
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
        .map_err(|error| format!("development server response failed: {error}"))
}
