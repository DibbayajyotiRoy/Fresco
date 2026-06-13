//! Unix-socket control server. Runs on its own thread and forwards each
//! request to the main render loop via a channel, returning the reply.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use anyhow::{anyhow, Result};

use crate::ipc::{socket_dir, socket_path, Request, Response};

/// A request paired with a channel to send its response back on.
pub type Command = (Request, Sender<Response>);

/// Bind the control socket and spawn the accept loop.
///
/// Doubles as the single-instance lock: if an existing socket answers a
/// connection, another daemon is alive and we return an error.
pub fn start_server() -> Result<Receiver<Command>> {
    let dir = socket_dir();
    std::fs::create_dir_all(&dir)?;
    let path = socket_path();

    if path.exists() {
        if UnixStream::connect(&path).is_ok() {
            return Err(anyhow!("another frescod instance is already running"));
        }
        // Stale socket from a crashed daemon — remove and rebind.
        std::fs::remove_file(&path).ok();
    }

    let listener = UnixListener::bind(&path)?;
    let (tx, rx) = channel::<Command>();

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            handle_conn(stream, &tx);
        }
    });

    Ok(rx)
}

fn handle_conn(mut stream: UnixStream, tx: &Sender<Command>) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let Ok(req) = serde_json::from_str::<Request>(line.trim()) else {
        return;
    };

    let (rtx, rrx) = channel::<Response>();
    if tx.send((req, rtx)).is_err() {
        return; // main loop gone
    }
    if let Ok(resp) = rrx.recv() {
        if let Ok(mut s) = serde_json::to_string(&resp) {
            s.push('\n');
            let _ = stream.write_all(s.as_bytes());
        }
    }
}
