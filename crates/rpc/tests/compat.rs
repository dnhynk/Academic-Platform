use academic_rpc::{
    FrameClass, PHASE1_PROTOCOL_POLICY, RpcError, ServerHandshakeConfig, authorize_mutable_request,
    decode_envelope_frame, encode_envelope_frame,
    generated::{
        ClientHandshake, LocalCoreEnvelope, MutableRequest, SyntheticIngestCommand,
        WriteDisposition, local_core_envelope, mutable_request,
    },
    negotiate_handshake,
};

fn client(major: u32, minor: u32, capabilities: &[&str]) -> ClientHandshake {
    ClientHandshake {
        protocol_name: "learning-platform.local-core".to_owned(),
        protocol_version: Some(academic_rpc::generated::ProtocolVersion { major, minor }),
        capability_ids: capabilities.iter().copied().map(str::to_owned).collect(),
    }
}

fn ingest_request() -> MutableRequest {
    MutableRequest {
        request_id: vec![1; 16],
        client_instance_id: vec![2; 16],
        idempotency_key: vec![3; 32],
        request_digest: vec![4; 32],
        expected_profile_revision: Some(0),
        capability_id: "learning-platform.local.synthetic-ingest.v1".to_owned(),
        command: Some(mutable_request::Command::SyntheticIngest(
            SyntheticIngestCommand {
                synthetic_fixture_id: "signed-batch-v2".to_owned(),
            },
        )),
    }
}

#[test]
fn major_version_mismatch_denies_write() -> Result<(), Box<dyn std::error::Error>> {
    let handshake = negotiate_handshake(
        &client(
            2,
            0,
            &[
                "learning-platform.local.synthetic-ingest.v1",
                "learning-platform.local.diagnostics.v1",
            ],
        ),
        &ServerHandshakeConfig::default(),
    )?;
    assert_eq!(
        handshake.write_disposition,
        WriteDisposition::DeniedMajorVersion as i32
    );
    assert_eq!(handshake.write_denial_reason, "MAJOR_VERSION_MISMATCH");
    assert_eq!(
        handshake.capability_ids,
        ["learning-platform.local.diagnostics.v1"]
    );
    assert_eq!(handshake.negotiated_protocol_version, None);
    assert!(matches!(
        authorize_mutable_request(&handshake, &ingest_request()),
        Err(RpcError::WriteDenied { .. })
    ));
    Ok(())
}

#[test]
fn unknown_write_capability_denies_write() -> Result<(), Box<dyn std::error::Error>> {
    let handshake = negotiate_handshake(
        &client(
            1,
            0,
            &[
                "learning-platform.local.synthetic-ingest.v1",
                "learning-platform.local.future-write.v9",
            ],
        ),
        &ServerHandshakeConfig::default(),
    )?;
    assert_eq!(
        handshake.write_disposition,
        WriteDisposition::DeniedUnknownCapability as i32
    );
    assert_eq!(handshake.write_denial_reason, "UNKNOWN_WRITE_CAPABILITY");
    assert_eq!(
        handshake.capability_ids,
        ["learning-platform.local.synthetic-ingest.v1"]
    );
    assert!(matches!(
        authorize_mutable_request(&handshake, &ingest_request()),
        Err(RpcError::WriteDenied { .. })
    ));
    Ok(())
}

#[test]
fn same_minor_capabilities_negotiate() -> Result<(), Box<dyn std::error::Error>> {
    let handshake = negotiate_handshake(
        &client(
            1,
            7,
            &[
                "learning-platform.local.synthetic-restore.v1",
                "learning-platform.local.synthetic-ingest.v1",
                "learning-platform.local.diagnostics.v1",
                "learning-platform.local.synthetic-ingest.v1",
            ],
        ),
        &ServerHandshakeConfig::default(),
    )?;
    assert_eq!(
        handshake.write_disposition,
        WriteDisposition::Allowed as i32
    );
    assert_eq!(handshake.write_denial_reason, "");
    assert_eq!(
        handshake.capability_ids,
        [
            "learning-platform.local.diagnostics.v1",
            "learning-platform.local.synthetic-ingest.v1",
            "learning-platform.local.synthetic-restore.v1",
        ]
    );
    assert_eq!(
        handshake.negotiated_protocol_version,
        Some(academic_rpc::generated::ProtocolVersion { major: 1, minor: 0 })
    );
    authorize_mutable_request(&handshake, &ingest_request())?;
    Ok(())
}

#[test]
fn handshake_round_trip_carries_exact_synthetic_policy() -> Result<(), Box<dyn std::error::Error>> {
    let response = negotiate_handshake(
        &client(1, 0, &["learning-platform.local.diagnostics.v1"]),
        &ServerHandshakeConfig::default(),
    )?;
    let envelope = LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::ServerHandshake(response)),
    };
    let frame = encode_envelope_frame(&envelope, FrameClass::Handshake)?;
    let decoded = decode_envelope_frame(&frame, FrameClass::Handshake)?;
    let Some(local_core_envelope::Payload::ServerHandshake(response)) = decoded.payload else {
        return Err("server handshake decoded into the wrong arm".into());
    };
    let policy = response.policy.ok_or("server policy was lost")?;
    assert_eq!(policy.data_policy, PHASE1_PROTOCOL_POLICY.data_policy);
    assert_eq!(policy.storage_mode, PHASE1_PROTOCOL_POLICY.storage_mode);
    assert_eq!(
        policy.storage_encryption,
        PHASE1_PROTOCOL_POLICY.storage_encryption
    );
    assert!(!policy.production_data_allowed);
    assert_eq!(
        policy.product_network,
        PHASE1_PROTOCOL_POLICY.product_network
    );
    assert!(policy.object_format.is_empty());
    assert!(policy.admission_receipt_digest.is_empty());
    assert!(policy.admission_platforms.is_empty());
    assert_eq!(
        policy.canonical_json,
        academic_admission::Posture::synthetic().canonical_json_bytes()
    );
    Ok(())
}
