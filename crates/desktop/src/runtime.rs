//! Native Tauri binding. Only versioned local commands cross this boundary.

use serde::{Deserialize, Serialize};

/// Closed frontend request vocabulary. Unknown fields and variants fail decoding.
#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeCommand {
    DetailsAudio {
        lecture_id: String,
        expected_profile_id: String,
        expected_revision: u64,
        selector: academic_rpc::details::DetailSelector,
        offset: u64,
        length: u64,
    },
    DetailsRead {
        selector: academic_rpc::details::DetailSelector,
    },
    DetailsDecide {
        relation_id: String,
        action: academic_rpc::details::DetailAction,
        expected_revision: u64,
        expected_profile_id: String,
        selector: academic_rpc::details::DetailSelector,
        request_id: [u8; 16],
        client_instance_id: [u8; 16],
        idempotency_key: [u8; 32],
    },
    Diagnostics {},
    SyntheticExport {},
    SyntheticIngest {},
    SyntheticBackup {},
    SyntheticRestore {
        backup_receipt_id: [u8; 16],
    },
}

impl RuntimeCommand {
    fn desktop_command(self) -> Option<crate::DesktopCommand> {
        match self {
            Self::DetailsRead { .. } | Self::DetailsDecide { .. } | Self::DetailsAudio { .. } => {
                None
            }
            Self::Diagnostics {} => Some(crate::DesktopCommand::Diagnostics),
            Self::SyntheticExport {} => Some(crate::DesktopCommand::SyntheticExport),
            Self::SyntheticIngest {} => Some(crate::DesktopCommand::SyntheticIngest(
                crate::SyntheticFixtureId::Phase1BitemporalLedgerV2,
            )),
            Self::SyntheticBackup {} => Some(crate::DesktopCommand::SyntheticBackup),
            Self::SyntheticRestore { backup_receipt_id } => {
                Some(crate::DesktopCommand::SyntheticRestore { backup_receipt_id })
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
    #[serde(flatten)]
    pub detail_fields: Option<RuntimeDetailFields>,
}

#[derive(Debug, Serialize)]
pub struct RuntimeDetailFields {
    pub decision_sequence: Option<u64>,
    pub receipt_decision: Option<academic_rpc::details::DetailDecisionReceipt>,
    pub reason: Option<String>,
    pub request_id: Option<[u8; 16]>,
    pub client_instance_id: Option<[u8; 16]>,
    pub idempotency_key: Option<[u8; 32]>,
    pub request_digest: Option<[u8; 32]>,
    pub details: Option<academic_rpc::details::DetailState>,
    pub audio: Option<academic_rpc::details::DetailAudio>,
}

impl RuntimeReply {
    pub(crate) fn unavailable() -> Self {
        Self {
            version: 1,
            state: "unavailable",
            message: "Save could not be confirmed. Check the local service before trying again.",
            receipt_id: None,
            detail_fields: None,
        }
    }
    pub(crate) fn state(state: &'static str, message: &'static str) -> Self {
        Self {
            version: 1,
            state,
            message,
            receipt_id: None,
            detail_fields: None,
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
    match request.operation {
        RuntimeCommand::DetailsAudio {
            lecture_id,
            expected_profile_id,
            expected_revision,
            selector,
            offset,
            length,
        } => Ok(client
            .execute_details(academic_rpc::details::DetailRequest::DetailsAudio {
                audio: academic_rpc::details::DetailAudioRequest {
                    lecture_id,
                    expected_profile_id,
                    expected_revision,
                    selector,
                    offset,
                    length,
                },
            })
            .await),
        RuntimeCommand::DetailsRead { selector } => Ok(client
            .execute_details(academic_rpc::details::DetailRequest::DetailsRead { selector })
            .await),
        RuntimeCommand::DetailsDecide {
            relation_id,
            action,
            expected_revision,
            expected_profile_id,
            selector,
            request_id,
            client_instance_id,
            idempotency_key,
        } => Ok(client
            .execute_details(academic_rpc::details::DetailRequest::DetailsDecide {
                decision: academic_rpc::details::DetailDecisionRequest {
                    relation_id,
                    action,
                    expected_revision,
                    expected_profile_id,
                    selector,
                    request_id,
                    client_instance_id,
                    idempotency_key,
                },
            })
            .await),
        operation => Ok(client
            .execute(
                operation
                    .desktop_command()
                    .ok_or("Unsupported desktop command")?,
            )
            .await),
    }
}

/// Starts one bundled local window with a fixed command manifest and no plugins.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let mut smoke = false;
    let mut session_path = None;
    while let Some(flag) = args.next() {
        if flag == "--smoke" && !smoke {
            smoke = true;
        } else if flag == "--session" && session_path.is_none() {
            session_path = Some(std::path::PathBuf::from(
                args.next().ok_or("--session requires daemon metadata")?,
            ));
        } else {
            return Err("Usage: academic-desktop [--session <session.meta>] [--smoke]".into());
        }
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
    fn detail_commands_require_profile_revision_identity_and_closed_selectors()
    -> Result<(), Box<dyn std::error::Error>> {
        let valid = serde_json::json!({"version": 1, "operation": {"command": "details_decide", "relation_id": "any-profile-relation", "action": "undo",
            "expected_revision": 3, "expected_profile_id": "selected-incarnation", "selector": {"view": "detail_workspace", "known_at_accept_seq": null, "valid_at_ms": null},
            "request_id": ([1; 16]), "client_instance_id": ([2; 16]), "idempotency_key": ([3; 32])}});
        let _: RuntimeRequest = serde_json::from_value(valid.clone())?;
        for field in [
            "expected_revision",
            "expected_profile_id",
            "selector",
            "request_id",
            "client_instance_id",
            "idempotency_key",
        ] {
            let mut missing = valid.clone();
            missing["operation"]
                .as_object_mut()
                .ok_or("operation object")?
                .remove(field);
            assert!(
                serde_json::from_value::<RuntimeRequest>(missing).is_err(),
                "missing {field}"
            );
        }
        let mut extra = valid.clone();
        extra["operation"]["path"] = serde_json::json!("not-allowed");
        assert!(serde_json::from_value::<RuntimeRequest>(extra).is_err());
        let mut extra = valid;
        extra["operation"]["action"] = serde_json::json!("confirm");
        assert!(serde_json::from_value::<RuntimeRequest>(extra).is_err());
        Ok(())
    }

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
