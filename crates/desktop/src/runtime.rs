//! Native Tauri binding. Only versioned local commands cross this boundary.

use serde::{Deserialize, Serialize};

/// Closed frontend request vocabulary. Unknown fields and variants fail decoding.
#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeCommand {
    Diagnostics {},
    SyntheticExport {},
    SyntheticIngest {},
    SyntheticBackup {},
    SyntheticRestore { backup_receipt_id: [u8; 16] },
}

impl RuntimeCommand {
    fn desktop_command(self) -> crate::DesktopCommand {
        match self {
            Self::Diagnostics {} => crate::DesktopCommand::Diagnostics,
            Self::SyntheticExport {} => crate::DesktopCommand::SyntheticExport,
            Self::SyntheticIngest {} => crate::DesktopCommand::SyntheticIngest(
                crate::SyntheticFixtureId::Phase1BitemporalLedgerV2,
            ),
            Self::SyntheticBackup {} => crate::DesktopCommand::SyntheticBackup,
            Self::SyntheticRestore { backup_receipt_id } => {
                crate::DesktopCommand::SyntheticRestore { backup_receipt_id }
            }
        }
    }
}

/// The frontend boundary version is independent of the daemon protocol version.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRequest {
    pub version: u16,
    pub operation: RuntimeCommand,
}

/// Availability never implies canonical acceptance.
#[derive(Debug, Serialize)]
pub struct RuntimeReply {
    pub version: u16,
    pub state: &'static str,
    pub message: &'static str,
    pub receipt_id: Option<Vec<u8>>,
}

impl RuntimeReply {
    pub(crate) fn unavailable() -> Self {
        Self {
            version: 1,
            state: "unavailable",
            message: "Save could not be confirmed. Check the local service before trying again.",
            receipt_id: None,
        }
    }
    pub(crate) fn state(state: &'static str, message: &'static str) -> Self {
        Self {
            version: 1,
            state,
            message,
            receipt_id: None,
        }
    }
}

#[tauri::command]
async fn desktop_request_v1(
    request: RuntimeRequest,
    client: tauri::State<'_, crate::local_client::LocalClient>,
) -> Result<RuntimeReply, &'static str> {
    if request.version != 1 {
        return Err("Unsupported desktop IPC version");
    }
    Ok(client.execute(request.operation.desktop_command()).await)
}

/// Starts one bundled local window with a fixed command manifest and no plugins.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let mut smoke = false;
    let session_path = match args.next() {
        None => None,
        Some(flag) if flag == "--smoke" => {
            smoke = true;
            None
        }
        Some(flag) if flag == "--session" => Some(std::path::PathBuf::from(
            args.next()
                .ok_or("--session requires the daemon metadata path")?,
        )),
        Some(_) => {
            return Err("Usage: academic-desktop [--session <daemon session metadata>]".into());
        }
    };
    if args.next().is_some() {
        return Err("Unexpected desktop argument".into());
    }
    tauri::Builder::default()
        .manage(crate::local_client::LocalClient::new(session_path))
        .on_page_load(move |webview, payload| {
            if smoke
                && payload.event() == tauri::webview::PageLoadEvent::Finished
                && let Err(error) =
                    webview.eval("import('./runtime-smoke.js').then(m => m.runSmoke())")
            {
                eprintln!("Native smoke could not start: {error}");
            }
        })
        .invoke_handler(tauri::generate_handler![desktop_request_v1])
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::RuntimeRequest;

    #[test]
    fn frontend_command_decoder_refuses_breadth() {
        for value in [
            r#"{"version":1,"operation":{"command":"shell"}}"#,
            r#"{"version":1,"operation":{"command":"synthetic_ingest","path":"secret.db"}}"#,
            r#"{"version":1,"operation":{"command":"diagnostics"},"session":"secret.db"}"#,
            r#"{"version":1,"operation":{"command":"synthetic_restore","backup_receipt_id":[0]}}"#,
        ] {
            assert!(serde_json::from_str::<RuntimeRequest>(value).is_err());
        }
        for command in [
            "diagnostics",
            "synthetic_export",
            "synthetic_ingest",
            "synthetic_backup",
        ] {
            let value = format!(r#"{{"version":1,"operation":{{"command":"{command}"}}}}"#);
            assert!(serde_json::from_str::<RuntimeRequest>(&value).is_ok());
        }
    }
}
