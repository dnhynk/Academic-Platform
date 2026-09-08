use academic_rpc::{
    FrameClass, decode_envelope_frame,
    details::*,
    encode_envelope_frame,
    generated::{DetailRequestFrame, LocalCoreEnvelope, local_core_envelope::Payload},
};

#[test]
fn original_decision_receipt_requires_canonical_v7_identifiers()
-> Result<(), Box<dyn std::error::Error>> {
    let claim = "01900000-0000-7000-8000-000000000911";
    let actor = "01900000-0000-7000-8000-000000000912";
    let receipt = DetailDecisionReceipt {
        sequence: 11,
        relation_id: "relation".to_owned(),
        relation_claim_id: claim.parse()?,
        action: DetailAction::Undo,
        undoes: Some(10),
        actor: actor.parse()?,
    };
    let bytes = encode(&receipt)?;
    assert_eq!(decode::<DetailDecisionReceipt>(&bytes)?, receipt);
    let text = String::from_utf8(bytes)?;
    for id in [claim, actor] {
        for invalid in ["01900000-0000-4000-8000-000000000999", "not-a-uuid"] {
            assert!(decode::<DetailDecisionReceipt>(text.replace(id, invalid).as_bytes()).is_err());
        }
    }
    Ok(())
}

#[test]
fn decision_digest_matches_the_shared_utf8_vector() -> Result<(), Box<dyn std::error::Error>> {
    let request = DetailDecisionRequest {
        relation_id: "example-relation".to_owned(),
        action: DetailAction::Reject,
        expected_revision: 1,
        expected_profile_id: "a".repeat(64),
        selector: DetailSelector::default(),
        request_id: [1; 16],
        client_instance_id: [2; 16],
        idempotency_key: [3; 32],
    };
    assert_eq!(
        decision_digest(&request)?.as_bytes(),
        &[
            36, 174, 215, 60, 199, 214, 224, 243, 200, 185, 123, 167, 28, 50, 92, 214, 73, 176,
            251, 210, 41, 236, 136, 219, 105, 31, 149, 60, 38, 21, 190, 63
        ]
    );
    Ok(())
}

#[test]
fn detail_commands_are_closed_and_canonical_on_the_wire() -> Result<(), Box<dyn std::error::Error>>
{
    let request = DetailRequest::DetailsRead {
        selector: DetailSelector::default(),
    };
    let envelope = LocalCoreEnvelope {
        payload: Some(Payload::DetailRequest(DetailRequestFrame {
            canonical_json: encode(&request)?,
        })),
    };
    assert_eq!(
        decode_envelope_frame(
            &encode_envelope_frame(&envelope, FrameClass::Command)?,
            FrameClass::Command
        )?,
        envelope
    );
    for bytes in [
        br#"{"command":"details_read","selector":{"view":"detail_workspace","known_at_accept_seq":null,"valid_at_ms":null},"path":"x"}"#.as_slice(),
        br#"{"command":"details_read","selector":{"view":"detail_workspace","known_at_accept_seq":9007199254740992,"valid_at_ms":null}}"#.as_slice(),
        br#"{"command":"details_read","selector":{"view":"detail_workspace","known_at_accept_seq":null,"valid_at_ms":null,"view":"detail_workspace"}}"#.as_slice(),
    ] { assert!(decode::<DetailRequest>(bytes).is_err()); }
    Ok(())
}

#[test]
fn every_semantic_decision_field_changes_the_digest() -> Result<(), Box<dyn std::error::Error>> {
    let request = DetailDecisionRequest {
        relation_id: "relation".to_owned(),
        action: DetailAction::Reject,
        expected_revision: 1,
        expected_profile_id: "profile".to_owned(),
        selector: DetailSelector::default(),
        request_id: [1; 16],
        client_instance_id: [2; 16],
        idempotency_key: [3; 32],
    };
    let original = decision_digest(&request)?;
    for index in 0..9 {
        let mut changed = request.clone();
        match index {
            0 => changed.relation_id.push('x'),
            1 => changed.action = DetailAction::Undo,
            2 => changed.expected_revision += 1,
            3 => changed.expected_profile_id.push('x'),
            4 => changed.selector.known_at_accept_seq = Some(1),
            5 => changed.selector.valid_at_ms = Some(1),
            6 => changed.request_id[0] += 1,
            7 => changed.client_instance_id[0] += 1,
            _ => changed.idempotency_key[0] += 1,
        }
        assert_ne!(decision_digest(&changed)?, original);
    }
    Ok(())
}
