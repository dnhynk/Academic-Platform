//! The process this crate's claim is measured against.
//!
//! It reproduces a process-class binary — one [`ProcessClass`], one
//! [`academic_process_sandbox::enter`] at the top of `main`, before any work —
//! and then asks the operating system what the process can still do. Proving
//! that the operating system refuses a socket means asking it for one, which is
//! why this file spells `TcpListener` and `TcpStream`; it is a `[[bin]]` with
//! `required-features = ["native-enforcement"]` and a path outside `src`, so no
//! default build produces it and no crate that depends on this one links it.
//!
//! # The two socket endpoints
//!
//! The loopback pair is the deterministic control and the one the acceptance
//! suite reads: a listener on `127.0.0.1:0` and a connect to whatever port it
//! was given need no network, no name service and no route, so an outcome other
//! than the declared one is the enforcement and not the host. The external pair
//! is optional, supplied on the command line, and is what a by-hand measurement
//! on a native host uses to say the same thing against a real route.
//!
//! # Usage
//!
//! `academic-process-sandbox-probe <CLASS> <scratch-dir> [<ip:port> <host:port>]`
//!
//! `<CLASS>` is a [`ProcessClass::as_str`] spelling. The exit code is `0` when
//! the class was enforced, `3` when the process refused to start, and `2` for a
//! malformed invocation.

use std::{
    io::Write as _,
    net::{TcpListener, TcpStream, ToSocketAddrs},
    path::Path,
    process::ExitCode,
    time::Duration,
};

use academic_policy::ProcessClass;

const EXISTING_FILE: &str = "probe-existing.txt";
const CREATED_FILE: &str = "probe-created.txt";

fn parse_class(value: &str) -> Option<ProcessClass> {
    ProcessClass::ALL
        .into_iter()
        .find(|class| class.as_str() == value)
}

/// Reports one operation as the outcome plus the platform error number, so a
/// refusal by the enforcement (`EPERM`, `EACCES`) is distinguishable from a
/// refusal by the host.
///
/// It borrows the error rather than taking it, because the listener below has
/// to stay open while the connect that targets it runs: consuming the `Result`
/// closed the socket and turned a permitted connect into `ECONNREFUSED`, which
/// is a host answer the suite would have read as an enforcement.
fn report(name: &str, outcome: Result<String, &std::io::Error>) {
    match outcome {
        Ok(detail) => println!("PROBE {name} = SUCCEEDED ({detail})"),
        Err(error) => println!(
            "PROBE {name} = REFUSED errno={} ({error})",
            error.raw_os_error().unwrap_or(-1)
        ),
    }
}

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let (Some(class_name), Some(scratch)) = (arguments.next(), arguments.next()) else {
        eprintln!("usage: probe <CLASS> <scratch-dir> [<ip:port> <host:port>]");
        return ExitCode::from(2);
    };
    let Some(class) = parse_class(&class_name) else {
        eprintln!("unknown process class {class_name}");
        return ExitCode::from(2);
    };
    let external_peer = arguments.next();
    let external_name = arguments.next();
    let scratch = Path::new(&scratch);

    // Setup, before the enforcement: the file the append below reopens.
    let existing = scratch.join(EXISTING_FILE);
    if let Err(error) = std::fs::write(&existing, b"t215") {
        eprintln!("probe could not stage {}: {error}", existing.display());
        return ExitCode::from(2);
    }

    println!("class = {}", class.as_str());
    println!("host = {}", std::env::consts::OS);
    println!("declared capabilities = {:?}", class.capabilities());

    // The whole of the process-class binary's body.
    let enforcement = match academic_process_sandbox::enter(class) {
        Ok(enforcement) => enforcement,
        Err(error) => {
            println!(
                "PROBE enter = REFUSED ({})",
                academic_process_sandbox::refusal_line(class, &error)
            );
            println!("PROBE operations = NOT_ATTEMPTED");
            return ExitCode::from(3);
        }
    };
    println!("PROBE enter = OK ({})", enforcement.receipt_line());

    let listener = TcpListener::bind("127.0.0.1:0");
    let bound = listener
        .as_ref()
        .ok()
        .and_then(|listener| listener.local_addr().ok());
    report(
        "net/listen",
        listener
            .as_ref()
            .map(|listener| format!("{:?}", listener.local_addr())),
    );

    // Whatever the listener was given, or the discard port when there was no
    // listener. Either way the address is on the loopback interface.
    let target = bound.unwrap_or_else(|| {
        std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::LOCALHOST,
            9,
        ))
    });
    let connected = TcpStream::connect_timeout(&target, Duration::from_secs(5));
    report(
        "net/connect-loopback",
        connected
            .as_ref()
            .map(|stream| format!("{:?}", stream.peer_addr())),
    );
    // The listener stays open until here, which is what the connect above
    // needed.
    drop(listener);

    match external_peer {
        Some(peer) => match peer.parse() {
            Ok(address) => {
                let external = TcpStream::connect_timeout(&address, Duration::from_secs(5));
                report(
                    "net/connect-external",
                    external
                        .as_ref()
                        .map(|stream| format!("{:?}", stream.peer_addr())),
                );
            }
            Err(error) => println!("PROBE net/connect-external = MALFORMED ({error})"),
        },
        None => println!("PROBE net/connect-external = NOT_ATTEMPTED"),
    }
    match external_name {
        Some(name) => {
            let resolved = name.to_socket_addrs();
            report(
                "net/resolve-external",
                resolved
                    .map(|mut addresses| format!("{:?}", addresses.next()))
                    .as_ref()
                    .map(String::clone),
            );
        }
        None => println!("PROBE net/resolve-external = NOT_ATTEMPTED"),
    }

    let created = scratch.join(CREATED_FILE);
    let written = std::fs::File::create(&created).and_then(|mut file| {
        file.write_all(b"t215")?;
        Ok(format!("{}", created.display()))
    });
    report("fs/write-new", written.as_ref().map(String::clone));
    // The handle is opened and nothing is written: a measurement must leave the
    // tree it measured byte-identical.
    let appended = std::fs::OpenOptions::new()
        .append(true)
        .open(&existing)
        .map(|_handle| format!("opened {} for append, wrote nothing", existing.display()));
    report("fs/append-existing", appended.as_ref().map(String::clone));

    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("Seccomp") || line.starts_with("NoNewPrivs") {
                println!("PROBE proc/{}", line.replace('\t', " "));
            }
        }
    }
    ExitCode::SUCCESS
}
