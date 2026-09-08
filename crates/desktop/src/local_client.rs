//! Existing daemon protocol over local IPC only. No profile or key is opened.
use crate::{DesktopCommand, Optimistic, SubmittedRequest, runtime::RuntimeReply};
use academic_rpc::{
    FrameClass, LOCAL_CORE_PROTOCOL_NAME, LOCAL_CORE_PROTOCOL_VERSION,
    generated::{
        ClientHandshake, LocalCoreEnvelope, MutableRequest, MutationStatus, ProfileLockState,
        ProtocolVersion, WriteDisposition, local_core_envelope::Payload,
    },
    read_envelope, write_envelope,
};
use std::{
    fmt,
    fs::File,
    io::{self, Read},
    path::PathBuf,
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncWrite};

/// Host-selected metadata file. The webview cannot select or change this path.
pub struct LocalClient {
    session_path: Option<PathBuf>,
    client_id: [u8; 16],
}
impl fmt::Debug for LocalClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalClient").finish_non_exhaustive()
    }
}
impl LocalClient {
    pub fn new(session_path: Option<PathBuf>) -> Self {
        Self {
            session_path,
            client_id: *uuid::Uuid::now_v7().as_bytes(),
        }
    }
    pub async fn execute(&self, command: DesktopCommand) -> RuntimeReply {
        // The deadline covers connection retries, both frames, and the response.
        match tokio::time::timeout(Duration::from_secs(5), self.exchange(command)).await {
            Ok(Ok(reply)) => reply,
            _ => RuntimeReply::unavailable(),
        }
    }
    fn session(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let path = self.session_path.as_ref().ok_or("No session selected")?;
        if path.file_name().is_none_or(|name| name != "session.meta") {
            return Err("Expected daemon session.meta".into());
        }
        let mut contents = String::new();
        File::open(path)?.take(4097).read_to_string(&mut contents)?;
        Ok(contents)
    }
    async fn exchange(
        &self,
        command: DesktopCommand,
    ) -> Result<RuntimeReply, Box<dyn std::error::Error + Send + Sync>> {
        // No filesystem path or session nonce ever crosses the JS boundary.
        let contents = self.session()?;
        let (endpoint, nonce) = parse_session(&contents)?;
        let mut stream = connect(endpoint).await?;
        self.protocol(command, &mut stream, nonce).await
    }
    async fn protocol(
        &self,
        command: DesktopCommand,
        mut stream: &mut Box<dyn ClientStream>,
        nonce: &str,
    ) -> Result<RuntimeReply, Box<dyn std::error::Error + Send + Sync>> {
        let handshake = ClientHandshake {
            protocol_name: LOCAL_CORE_PROTOCOL_NAME.to_owned(),
            protocol_version: Some(ProtocolVersion {
                major: u32::from(LOCAL_CORE_PROTOCOL_VERSION.major),
                minor: u32::from(LOCAL_CORE_PROTOCOL_VERSION.minor),
            }),
            capability_ids: vec![
                command.capability_id().to_owned(),
                format!("learning-platform.local.session-nonce.{nonce}"),
            ],
        };
        write_envelope(
            &mut stream,
            &LocalCoreEnvelope {
                payload: Some(Payload::ClientHandshake(handshake)),
            },
            FrameClass::Handshake,
        )
        .await?;
        let Some(Payload::ServerHandshake(handshake)) =
            read_envelope(&mut stream, FrameClass::Handshake)
                .await?
                .payload
        else {
            return Err("Wrong handshake frame".into());
        };
        if handshake.lock_state != ProfileLockState::Unlocked as i32 {
            return Ok(RuntimeReply::state(
                "locked",
                "Local profile is locked or requires repair. Nothing has been saved.",
            ));
        }
        if handshake.protocol_name != LOCAL_CORE_PROTOCOL_NAME
            || handshake
                .negotiated_protocol_version
                .as_ref()
                .is_none_or(|v| v.major != u32::from(LOCAL_CORE_PROTOCOL_VERSION.major))
            || handshake.write_disposition != WriteDisposition::Allowed as i32
            || !handshake
                .capability_ids
                .iter()
                .any(|id| id == command.capability_id())
        {
            return Ok(RuntimeReply::state(
                "incompatible",
                "Local service is incompatible with this command. Nothing has been saved.",
            ));
        }
        if handshake
            .policy
            .as_ref()
            .is_none_or(|policy| policy.production_data_allowed)
        {
            return Err("Synthetic runtime requires synthetic daemon".into());
        }
        if command == DesktopCommand::Diagnostics {
            return Ok(RuntimeReply::state(
                "ready",
                "Local service connected and unlocked. Synthetic data only; no save requested.",
            ));
        }
        let Some(wire_command) = command.mutable_command() else {
            return Ok(RuntimeReply::state(
                "unsupported",
                "The daemon protocol has no export response. Use the local CLI export workflow.",
            ));
        };
        let request_id = *uuid::Uuid::now_v7().as_bytes();
        let mut idempotency_key = [0_u8; 32];
        idempotency_key[..16].copy_from_slice(&request_id);
        idempotency_key[16..].copy_from_slice(&self.client_id);
        let mut request = MutableRequest {
            request_id: request_id.to_vec(),
            client_instance_id: self.client_id.to_vec(),
            idempotency_key: idempotency_key.to_vec(),
            request_digest: vec![0; 32],
            expected_profile_revision: None,
            capability_id: command.capability_id().to_owned(),
            command: Some(wire_command),
        };
        let digest = academic_rpc::digest::mutable_request_digest(&request)?;
        request.request_digest = digest.as_bytes().to_vec();
        let pending = Optimistic::new(
            (),
            SubmittedRequest {
                request_id,
                client_instance_id: self.client_id,
                idempotency_key,
                request_digest: *digest.as_bytes(),
            },
        );
        // No retry occurs after this first send: delivery can become ambiguous.
        write_envelope(
            &mut stream,
            &LocalCoreEnvelope {
                payload: Some(Payload::MutableRequest(request)),
            },
            FrameClass::Command,
        )
        .await?;
        let Some(Payload::MutableResponse(response)) =
            read_envelope(&mut stream, FrameClass::Command)
                .await?
                .payload
        else {
            return Err("Wrong response frame".into());
        };
        if response.request_id != request_id {
            return Err("Response does not identify the submitted request".into());
        }
        if !matches!(
            MutationStatus::try_from(response.status),
            Ok(MutationStatus::Accepted | MutationStatus::Duplicate)
        ) {
            return Ok(RuntimeReply::state(
                "rejected",
                "The local service declined this request. Nothing has been saved by this request.",
            ));
        }
        let receipt = response.receipt.ok_or("Missing core receipt")?;
        let canonical = pending.confirm(&receipt)?;
        Ok(RuntimeReply {
            version: 1,
            state: "accepted",
            message: "Synthetic example saved.",
            receipt_id: Some(canonical.receipt().receipt_id.clone()),
            detail_fields: None,
        })
    }

    pub async fn execute_details(
        &self,
        request: academic_rpc::details::DetailRequest,
    ) -> RuntimeReply {
        match tokio::time::timeout(Duration::from_secs(10), self.exchange_details(request)).await {
            Ok(Ok(reply)) => reply,
            _ => RuntimeReply::unavailable(),
        }
    }
    async fn exchange_details(
        &self,
        request: academic_rpc::details::DetailRequest,
    ) -> Result<RuntimeReply, Box<dyn std::error::Error + Send + Sync>> {
        let academic_rpc::domain_details::wire::DetailFrameReply::Imported(reply) = self
            .exchange_detail_frame(
                &academic_rpc::domain_details::wire::DetailFrameRequest::Imported(request.clone()),
            )
            .await?
        else {
            return Err("Wrong imported detail response".into());
        };
        self.imported_reply(request, *reply)
    }

    pub async fn execute_domain_details(
        &self,
        request: academic_rpc::domain_details::DomainReadRequest,
    ) -> academic_rpc::domain_details::DomainReadReply {
        use academic_rpc::domain_details::{
            DomainReadReply, ReadFailure,
            wire::{DetailFrameReply, DetailFrameRequest},
        };
        if request.validate().is_err() {
            return DomainReadReply::unavailable(ReadFailure::UnsupportedQuery);
        }
        let frame = DetailFrameRequest::Domain(request.clone());
        match tokio::time::timeout(Duration::from_secs(10), self.exchange_detail_frame(&frame))
            .await
        {
            Ok(Ok(DetailFrameReply::Domain(reply))) if reply.validate_for(&request).is_ok() => {
                reply
            }
            _ => DomainReadReply::unavailable(ReadFailure::CapabilityUnavailable),
        }
    }

    async fn exchange_detail_frame(
        &self,
        request: &academic_rpc::domain_details::wire::DetailFrameRequest,
    ) -> Result<
        academic_rpc::domain_details::wire::DetailFrameReply,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        use academic_rpc::details as dto;
        request.validate()?;
        let contents = self.session()?;
        let (endpoint, nonce) = parse_session(&contents)?;
        let mut stream = connect(endpoint).await?;
        write_envelope(
            &mut stream,
            &LocalCoreEnvelope {
                payload: Some(Payload::ClientHandshake(ClientHandshake {
                    protocol_name: LOCAL_CORE_PROTOCOL_NAME.to_owned(),
                    protocol_version: Some(ProtocolVersion { major: 1, minor: 0 }),
                    capability_ids: vec![
                        request.capability().to_owned(),
                        format!("learning-platform.local.session-nonce.{nonce}"),
                    ],
                })),
            },
            FrameClass::Handshake,
        )
        .await?;
        let Some(Payload::ServerHandshake(handshake)) =
            read_envelope(&mut stream, FrameClass::Handshake)
                .await?
                .payload
        else {
            return Err("Wrong handshake".into());
        };
        if handshake.protocol_name != LOCAL_CORE_PROTOCOL_NAME
            || handshake.write_disposition != WriteDisposition::Allowed as i32
            || handshake.lock_state != ProfileLockState::Unlocked as i32
            || handshake
                .negotiated_protocol_version
                .as_ref()
                .is_none_or(|v| v.major != 1)
            || !handshake
                .capability_ids
                .iter()
                .any(|id| id == request.capability())
            || handshake
                .policy
                .as_ref()
                .is_none_or(|p| p.production_data_allowed)
        {
            return Err("Detail capability unavailable".into());
        }
        write_envelope(
            &mut stream,
            &LocalCoreEnvelope {
                payload: Some(Payload::DetailRequest(
                    academic_rpc::generated::DetailRequestFrame {
                        canonical_json: dto::encode(&request)?,
                    },
                )),
            },
            FrameClass::Command,
        )
        .await?;
        let Some(Payload::DetailResponse(frame)) = read_envelope(&mut stream, FrameClass::Command)
            .await?
            .payload
        else {
            return Err("Wrong detail response".into());
        };
        let reply: academic_rpc::domain_details::wire::DetailFrameReply =
            dto::decode(&frame.canonical_json)?;
        reply.validate()?;
        Ok(reply)
    }

    fn imported_reply(
        &self,
        request: academic_rpc::details::DetailRequest,
        reply: academic_rpc::details::DetailReply,
    ) -> Result<RuntimeReply, Box<dyn std::error::Error + Send + Sync>> {
        use academic_rpc::details as dto;
        if reply.version != 1 {
            return Err("Wrong detail response version".into());
        }
        if let dto::DetailRequest::DetailsDecide { decision } = &request {
            validate_detail_decision_reply(decision, &reply)?;
        }
        if let dto::DetailRequest::DetailsAudio { audio: request } = &request
            && reply.state == dto::DetailReplyState::Ready
        {
            let audio = reply.audio.as_ref().ok_or("missing audio response")?;
            if audio.lecture_id != request.lecture_id
                || audio.offset != request.offset
                || audio.bytes.is_empty()
                || u64::try_from(audio.bytes.len())? > request.length
                || audio.offset + u64::try_from(audio.bytes.len())? > audio.total_bytes
            {
                return Err("audio response range mismatch".into());
            }
        }
        Ok(RuntimeReply {
            version: 1,
            state: match reply.state {
                dto::DetailReplyState::Ready => "ready",
                dto::DetailReplyState::Accepted => "accepted",
                dto::DetailReplyState::Rejected => "rejected",
                dto::DetailReplyState::Unavailable => "unavailable",
            },
            message: "Local detail service response.",
            receipt_id: reply.receipt_id.map(|id| id.to_vec()),
            detail_fields: Some(crate::runtime::RuntimeDetailFields {
                decision_sequence: reply.decision_sequence,
                receipt_decision: reply.receipt_decision,
                reason: reply.reason,
                request_id: reply.request_id,
                client_instance_id: reply.client_instance_id,
                idempotency_key: reply.idempotency_key,
                request_digest: reply.request_digest,
                details: reply.details,
                audio: reply.audio,
            }),
        })
    }
}

fn validate_detail_decision_reply(
    request: &academic_rpc::details::DetailDecisionRequest,
    reply: &academic_rpc::details::DetailReply,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use academic_rpc::details as dto;
    reply.validate()?;
    if reply.request_id != Some(request.request_id)
        || reply.client_instance_id != Some(request.client_instance_id)
        || reply.idempotency_key != Some(request.idempotency_key)
        || reply.request_digest != Some(*dto::decision_digest(request)?.as_bytes())
    {
        return Err("Detail receipt identity mismatch".into());
    }
    if reply.state == dto::DetailReplyState::Accepted {
        if reply.details.as_ref().is_none_or(|state| {
            state.revision <= request.expected_revision
                || state.profile_id != request.expected_profile_id
        }) {
            return Err("Detail response has no matching durable revision/profile".into());
        }
        let original = reply
            .receipt_decision
            .as_ref()
            .ok_or("missing original decision receipt")?;
        if Some(original.sequence) != reply.decision_sequence
            || original.relation_id != request.relation_id
            || original.action != request.action
        {
            return Err("original receipt decision does not match request".into());
        }
    }
    Ok(())
}

fn parse_session(contents: &str) -> Result<(&str, &str), &'static str> {
    if contents.len() > 4096 {
        return Err("Session metadata too large");
    }
    let lines: Vec<_> = contents.lines().collect();
    let ["version=1", endpoint, nonce] = lines.as_slice() else {
        return Err("Invalid session metadata");
    };
    let endpoint = endpoint
        .strip_prefix("endpoint=")
        .ok_or("Missing endpoint")?;
    let nonce = nonce.strip_prefix("nonce=").ok_or("Missing nonce")?;
    if nonce.len() != 64
        || !nonce
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("Invalid session nonce");
    }
    #[cfg(windows)]
    {
        let rest = endpoint
            .strip_prefix(r"\\.\pipe\academic-os\")
            .ok_or("Non-local endpoint")?;
        let (session, profile) = rest.split_once('\\').ok_or("Invalid endpoint")?;
        if session.is_empty()
            || !session.bytes().all(|b| b.is_ascii_digit())
            || profile.len() != 16
            || !profile.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Invalid endpoint");
        }
    }
    #[cfg(unix)]
    if !std::path::Path::new(endpoint).is_absolute()
        || !endpoint.ends_with("/d.sock")
        || endpoint.split('/').any(|part| part == "..")
    {
        return Err("Non-local endpoint");
    }
    Ok((endpoint, nonce))
}
trait ClientStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> ClientStream for T {}
async fn connect(endpoint: &str) -> io::Result<Box<dyn ClientStream>> {
    #[cfg(windows)]
    {
        connect_with_retry(
            || tokio::net::windows::named_pipe::ClientOptions::new().open(endpoint),
            std::time::Instant::now,
            tokio::time::sleep,
        )
        .await
        .map(|stream| Box::new(stream) as Box<dyn ClientStream>)
    }
    #[cfg(unix)]
    Ok(Box::new(tokio::net::UnixStream::connect(endpoint).await?))
}

// Check the clock before every open: an executor may resume a sleep after its
// deadline, and Tokio timeout polls the inner future before checking its timer.
#[cfg(any(windows, test))]
async fn connect_with_retry<T, F: std::future::Future<Output = ()>>(
    mut open: impl FnMut() -> io::Result<T>,
    mut now: impl FnMut() -> std::time::Instant,
    mut sleep: impl FnMut(Duration) -> F,
) -> io::Result<T> {
    let deadline = now() + Duration::from_millis(500);
    for attempt in 0..25 {
        if now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Local connection deadline exceeded",
            ));
        }
        match open() {
            Ok(stream) => return Ok(stream),
            Err(error) if matches!(error.raw_os_error(), Some(2 | 231)) && attempt < 24 => {
                let remaining = deadline.saturating_duration_since(now());
                sleep(Duration::from_millis(20).min(remaining)).await;
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other(
        "Local connection attempt budget exhausted",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use academic_rpc::{
        ServerHandshakeConfig,
        generated::{ImmutableReceipt, MutableResponse},
        negotiate_handshake,
    };

    #[test]
    fn original_receipt_can_be_confirmed_after_visible_source_removal()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use academic_rpc::details as dto;
        let request = dto::DetailDecisionRequest {
            relation_id: "removed-relation".to_owned(),
            action: dto::DetailAction::Reject,
            expected_revision: 1,
            expected_profile_id: "profile-incarnation".to_owned(),
            selector: dto::DetailSelector::default(),
            request_id: [1; 16],
            client_instance_id: [2; 16],
            idempotency_key: [3; 32],
        };
        let reply: dto::DetailReply = serde_json::from_value(serde_json::json!({
            "version":1,"state":"accepted","message":"ACCEPTED","receipt_id":([4;16]),"decision_sequence":11,
            "receipt_decision":{"sequence":11,"relation_id":"removed-relation","relation_claim_id":"01900000-0000-7000-8000-000000000911","action":"reject","undoes":null,"actor":"01900000-0000-7000-8000-000000000912"},
            "reason":"ACCEPTED","request_id":request.request_id,"client_instance_id":request.client_instance_id,"idempotency_key":request.idempotency_key,"request_digest":dto::decision_digest(&request)?.as_bytes(),
            "details":{"corpus":{"lectures":[],"concepts":[],"projects":[],"questions":[]},"decisions":[],"revision":3,"profile_id":"profile-incarnation","known_at_accept_seq":15,"valid_at_ms":200,"projector_version":"academic.details.v1","source_digest":"source"},"audio":null
        }))?;
        validate_detail_decision_reply(&request, &reply)?;
        let mut undo_request = request.clone();
        undo_request.action = dto::DetailAction::Undo;
        let mut undo_reply = reply.clone();
        undo_reply.request_digest = Some(*dto::decision_digest(&undo_request)?.as_bytes());
        let original_undo = undo_reply
            .receipt_decision
            .as_mut()
            .ok_or("missing undo receipt")?;
        original_undo.action = dto::DetailAction::Undo;
        original_undo.undoes = Some(5);
        validate_detail_decision_reply(&undo_request, &undo_reply)?;
        for target in [None, Some(0), Some(11)] {
            let mut invalid = undo_reply.clone();
            invalid
                .receipt_decision
                .as_mut()
                .ok_or("missing undo receipt")?
                .undoes = target;
            assert!(validate_detail_decision_reply(&undo_request, &invalid).is_err());
        }
        for index in 0..5 {
            let mut bad = reply.clone();
            match index {
                0 => bad.request_id = Some([9; 16]),
                1 => bad.decision_sequence = Some(12),
                2 => {
                    bad.receipt_decision
                        .as_mut()
                        .ok_or("missing test receipt")?
                        .action = dto::DetailAction::Undo
                }
                3 => {
                    bad.receipt_decision
                        .as_mut()
                        .ok_or("missing test receipt")?
                        .relation_id = "another-relation".to_owned()
                }
                _ => bad.receipt_decision = None,
            }
            assert!(validate_detail_decision_reply(&request, &bad).is_err());
        }
        Ok(())
    }

    #[tokio::test]
    async fn delayed_retry_poll_never_opens_after_deadline() {
        let start = std::time::Instant::now();
        let clock = std::cell::Cell::new(start);
        let opens = std::cell::Cell::new(0);
        let result = connect_with_retry(
            || {
                opens.set(opens.get() + 1);
                if opens.get() == 1 {
                    // The first open itself uses most of the remaining budget.
                    clock.set(start + Duration::from_millis(490));
                    Err(io::Error::from_raw_os_error(231))
                } else {
                    Ok(()) // A late second open would incorrectly succeed.
                }
            },
            || clock.get(),
            |delay| {
                assert_eq!(delay, Duration::from_millis(10));
                // Model an executor resuming the retry after the sleep is due.
                clock.set(start + Duration::from_millis(501));
                std::future::ready(())
            },
        )
        .await;
        assert_eq!(
            result.err().map(|error| error.kind()),
            Some(io::ErrorKind::TimedOut)
        );
        assert_eq!(opens.get(), 1);
    }

    #[tokio::test]
    async fn retry_budget_and_nontransient_errors_remain_bounded() {
        for (code, expected_opens) in [(2, 25), (231, 25), (5, 1)] {
            let start = std::time::Instant::now();
            let clock = std::cell::Cell::new(start);
            let opens = std::cell::Cell::new(0);
            let result: io::Result<()> = connect_with_retry(
                || {
                    opens.set(opens.get() + 1);
                    Err(io::Error::from_raw_os_error(code))
                },
                || clock.get(),
                |delay| {
                    clock.set(clock.get() + delay);
                    std::future::ready(())
                },
            )
            .await;
            assert_eq!(
                result.err().and_then(|error| error.raw_os_error()),
                Some(code)
            );
            assert_eq!(opens.get(), expected_opens);
        }
    }

    #[test]
    fn session_metadata_is_bounded_and_local() {
        #[cfg(windows)]
        let endpoint = r"\\.\pipe\academic-os\1\0123456789abcdef";
        #[cfg(unix)]
        let endpoint = "/run/user/1000/academic-os/0123456789abcdef/d.sock";
        let valid = format!("version=1\nendpoint={endpoint}\nnonce={}\n", "a".repeat(64));
        assert!(parse_session(&valid).is_ok());
        for invalid in [
            valid.replace("version=1", "version=2"),
            valid.replace(endpoint, "https://example.invalid"),
            format!("{valid}nonce={}\n", "b".repeat(64)),
            "x".repeat(4097),
        ] {
            assert!(parse_session(&invalid).is_err());
        }
    }

    async fn round_trip(
        lock_state: ProfileLockState,
        mismatched_receipt: bool,
        drop_ack: bool,
    ) -> Result<RuntimeReply, Box<dyn std::error::Error + Send + Sync>> {
        let (client_stream, mut server) = tokio::io::duplex(65536);
        let client = LocalClient::new(None);
        let mut stream: Box<dyn ClientStream> = Box::new(client_stream);
        let server_task = async move {
            let Some(Payload::ClientHandshake(mut hello)) =
                read_envelope(&mut server, FrameClass::Handshake)
                    .await?
                    .payload
            else {
                return Err("Missing client handshake".into());
            };
            assert_eq!(
                hello.capability_ids.pop(),
                Some(format!(
                    "learning-platform.local.session-nonce.{}",
                    "a".repeat(64)
                ))
            );
            let hello = negotiate_handshake(
                &hello,
                &ServerHandshakeConfig {
                    lock_state,
                    ..ServerHandshakeConfig::default()
                },
            )?;
            write_envelope(
                &mut server,
                &LocalCoreEnvelope {
                    payload: Some(Payload::ServerHandshake(hello)),
                },
                FrameClass::Handshake,
            )
            .await?;
            if lock_state != ProfileLockState::Unlocked {
                return Ok::<(), Box<dyn std::error::Error + Send + Sync>>(());
            }
            let Some(Payload::MutableRequest(request)) =
                read_envelope(&mut server, FrameClass::Command)
                    .await?
                    .payload
            else {
                return Err("Missing mutation".into());
            };
            assert_eq!(
                request.request_digest,
                academic_rpc::digest::mutable_request_digest(&request)?.as_bytes()
            );
            if drop_ack {
                return Ok(());
            }
            let mut receipt = ImmutableReceipt {
                receipt_id: vec![7; 16],
                request_id: request.request_id.clone(),
                client_instance_id: request.client_instance_id,
                idempotency_key: request.idempotency_key,
                request_digest: request.request_digest,
                profile_revision: 1,
                acceptance_range: None,
            };
            if mismatched_receipt {
                receipt.client_instance_id[0] ^= 1;
            }
            let response = MutableResponse {
                request_id: request.request_id,
                status: MutationStatus::Accepted as i32,
                reason: "SYNTHETIC_ACCEPTED".to_owned(),
                receipt: Some(receipt),
                profile_revision: 1,
                acceptance_range: None,
                response_digest: vec![8; 32],
            };
            write_envelope(
                &mut server,
                &LocalCoreEnvelope {
                    payload: Some(Payload::MutableResponse(response)),
                },
                FrameClass::Command,
            )
            .await?;
            Ok(())
        };
        let nonce = "a".repeat(64);
        let (reply, served) = tokio::join!(
            client.protocol(DesktopCommand::SyntheticBackup, &mut stream, &nonce),
            server_task
        );
        served?;
        reply
    }

    #[tokio::test]
    async fn runtime_requires_matching_core_ack_and_surfaces_locked_state()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        assert_eq!(
            round_trip(ProfileLockState::Unlocked, false, false)
                .await?
                .state,
            "accepted"
        );
        assert!(
            round_trip(ProfileLockState::Unlocked, true, false)
                .await
                .is_err()
        );
        assert!(
            round_trip(ProfileLockState::Unlocked, false, true)
                .await
                .is_err()
        );
        let locked = round_trip(ProfileLockState::Locked, false, false).await?;
        assert_eq!(locked.state, "locked");
        assert!(locked.receipt_id.is_none());
        Ok(())
    }
}
