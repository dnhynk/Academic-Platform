pub mod support;
use academic_daemon::RunningDaemon;
use academic_rpc::{
    FrameClass, LOCAL_CORE_PROTOCOL_NAME,
    details::*,
    generated::{
        ClientHandshake, DetailRequestFrame, LocalCoreEnvelope, ProtocolVersion,
        local_core_envelope::Payload,
    },
    read_envelope, write_envelope,
};
use std::{collections::BTreeMap, error::Error};
use support::{TestEnvironment, complete_handshake, connect};

async fn exchange(
    daemon: &RunningDaemon,
    request: DetailRequest,
) -> Result<DetailReply, Box<dyn Error>> {
    let mut stream = connect(daemon.endpoint()).await?;
    complete_handshake(
        &mut stream,
        ClientHandshake {
            protocol_name: LOCAL_CORE_PROTOCOL_NAME.to_owned(),
            protocol_version: Some(ProtocolVersion { major: 1, minor: 0 }),
            capability_ids: vec![
                request.capability().to_owned(),
                daemon.session_nonce().capability_id(),
            ],
        },
    )
    .await?;
    write_envelope(
        &mut stream,
        &LocalCoreEnvelope {
            payload: Some(Payload::DetailRequest(DetailRequestFrame {
                canonical_json: encode(&request)?,
            })),
        },
        FrameClass::Command,
    )
    .await?;
    let Some(Payload::DetailResponse(response)) = read_envelope(&mut stream, FrameClass::Command)
        .await?
        .payload
    else {
        return Err("unexpected response".into());
    };
    Ok(decode(&response.canonical_json)?)
}
fn source() -> DetailCorpus {
    DetailCorpus {
        concepts: vec![Concept {
            id: "concept".to_owned(),
            title: "Synthetic concept".to_owned(),
            state: "UNKNOWN".to_owned(),
            confidence: "Unknown".to_owned(),
            freshness: "UNKNOWN".to_owned(),
            last_strong_evidence: "None".to_owned(),
            explanation: String::new(),
            relations: BTreeMap::from([(
                "Evidence".to_owned(),
                vec![Relation {
                    id: "relation".to_owned(),
                    label: "Evidence".to_owned(),
                    source: Source {
                        id: "source".to_owned(),
                        title: "Synthetic".to_owned(),
                        locator: "text bytes".to_owned(),
                        content: "SYNTHETIC daemon evidence".to_owned(),
                        href: None,
                    },
                    status: RelationStatus::Proposed,
                    confidence: "Unknown".to_owned(),
                    target: None,
                }],
            )]),
        }],
        ..DetailCorpus::default()
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_ipc_concurrent_revision_and_restart_replay() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("details")?;
    academic_core::details::fixture::import_synthetic_corpus(&profile, source())?;
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;
    let read = DetailRequest::DetailsRead {
        selector: DetailSelector::default(),
    };
    let state = exchange(&daemon, read.clone())
        .await?
        .details
        .ok_or("missing details")?;
    assert_eq!(state.corpus, source());
    let first = DetailDecisionRequest {
        relation_id: "relation".to_owned(),
        action: DetailAction::Reject,
        expected_revision: state.revision,
        expected_profile_id: state.profile_id,
        selector: DetailSelector::default(),
        request_id: [1; 16],
        client_instance_id: [2; 16],
        idempotency_key: [3; 32],
    };
    let mut second = first.clone();
    second.request_id = [4; 16];
    second.idempotency_key = [5; 32];
    let (a, b) = tokio::join!(
        exchange(
            &daemon,
            DetailRequest::DetailsDecide {
                decision: first.clone()
            }
        ),
        exchange(
            &daemon,
            DetailRequest::DetailsDecide {
                decision: second.clone()
            }
        )
    );
    let a = a?;
    let b = b?;
    let (winner, winner_request, loser) = if a.state == DetailReplyState::Accepted {
        (a, first, b)
    } else {
        (b, second, a)
    };
    assert_eq!(loser.state, DetailReplyState::Rejected);
    assert_eq!(loser.reason.as_deref(), Some("REVISION_CONFLICT"));
    assert_eq!(winner.details.as_ref().map(|s| s.decisions.len()), Some(1));
    daemon.shutdown().await?;
    let restarted = RunningDaemon::start(environment.config(&profile)).await?;
    let retry = exchange(
        &restarted,
        DetailRequest::DetailsDecide {
            decision: winner_request,
        },
    )
    .await?;
    assert_eq!(retry.receipt_id, winner.receipt_id);
    assert_eq!(retry.decision_sequence, winner.decision_sequence);
    assert_eq!(
        retry.details.as_ref().map(|s| &s.decisions),
        winner.details.as_ref().map(|s| &s.decisions)
    );
    let snapshot = exchange(&restarted, read)
        .await?
        .details
        .ok_or("missing restart details")?;
    assert_eq!(snapshot.revision, 2);
    restarted.shutdown().await?;
    Ok(())
}
