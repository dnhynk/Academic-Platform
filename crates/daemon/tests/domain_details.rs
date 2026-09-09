pub mod support;
use academic_daemon::RunningDaemon;
use academic_rpc::{
    FrameClass, LOCAL_CORE_PROTOCOL_NAME,
    details::{decode, encode},
    domain_details::*,
    generated::{
        ClientHandshake, DetailRequestFrame, LocalCoreEnvelope, ProtocolVersion,
        local_core_envelope::Payload,
    },
    read_envelope, write_envelope,
};
use std::error::Error;
use support::{TestEnvironment, complete_handshake, connect};

async fn exchange(
    daemon: &RunningDaemon,
    request: &DomainReadRequest,
) -> Result<DomainReadReply, Box<dyn Error>> {
    let mut stream = connect(daemon.endpoint()).await?;
    complete_handshake(
        &mut stream,
        ClientHandshake {
            protocol_name: LOCAL_CORE_PROTOCOL_NAME.into(),
            protocol_version: Some(ProtocolVersion { major: 1, minor: 0 }),
            capability_ids: vec![CAPABILITY.into(), daemon.session_nonce().capability_id()],
        },
    )
    .await?;
    write_envelope(
        &mut stream,
        &LocalCoreEnvelope {
            payload: Some(Payload::DetailRequest(DetailRequestFrame {
                canonical_json: encode(request)?,
            })),
        },
        FrameClass::Command,
    )
    .await?;
    let Some(Payload::DetailResponse(frame)) = read_envelope(&mut stream, FrameClass::Command)
        .await?
        .payload
    else {
        return Err("unexpected domain read response".into());
    };
    let reply: DomainReadReply = decode(&frame.canonical_json)?;
    reply.validate_for(request)?;
    Ok(reply)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn domain_command_refuses_unknown_context_and_exact_future_coordinate_without_mutating()
-> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("domain-read-unavailable")?;
    let before = academic_store::queries::canonical_snapshot(&profile.open_reader()?)?;
    let mut request = DomainReadRequest::DetailsDomainReadV3 {
        context: Context {
            domain_id: "01900000-0000-7000-8000-000000000003".parse()?,
            scope_id: "01900000-0000-7000-8000-000000000004".parse()?,
        },
        selector: DomainSelector {
            view: DomainView::DomainDetailV3,
            known_at_accept_seq: None,
            valid_at_ms: Some(50),
        },
        query: Query::Index {
            surface: Surface::Concept,
        },
    };
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;
    let reply = exchange(&daemon, &request).await?;
    assert_eq!(
        reply,
        DomainReadReply::unavailable(ReadFailure::ContextUnavailable)
    );
    let value = String::from_utf8(encode(&reply)?)?;
    assert!(!value.contains("receipt_decision"));
    assert!(!value.contains("receipt_id"));
    let DomainReadRequest::DetailsDomainReadV3 { selector, .. } = &mut request;
    selector.known_at_accept_seq = Some(1);
    assert_eq!(
        exchange(&daemon, &request).await?,
        DomainReadReply::unavailable(ReadFailure::SelectorUnavailable)
    );
    daemon.shutdown().await?;
    let restarted = RunningDaemon::start(environment.config(&profile)).await?;
    assert_eq!(
        exchange(&restarted, &request).await?,
        DomainReadReply::unavailable(ReadFailure::SelectorUnavailable)
    );
    restarted.shutdown().await?;
    assert_eq!(
        academic_store::queries::canonical_snapshot(&profile.open_reader()?)?,
        before
    );
    Ok(())
}
