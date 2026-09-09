use academic_rpc::{
    FrameClass, PHASE1_PROTOCOL_POLICY, RpcError, ServerHandshakeConfig, authorize_mutable_request,
    decode_envelope_frame, encode_envelope_frame,
    generated::{
        ClientHandshake, LocalCoreEnvelope, MutableRequest, ProfileLockState, ServerHandshake,
        SyntheticIngestCommand, WriteDisposition, local_core_envelope, mutable_request,
    },
    handshake::ENCRYPTED_SYNTHETIC_POSTURE_CAPABILITY,
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

fn server_envelope(server: ServerHandshake) -> LocalCoreEnvelope {
    LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::ServerHandshake(server)),
    }
}

#[test]
fn encrypted_identity_round_trip_is_unavailable_and_grants_no_commands()
-> Result<(), Box<dyn std::error::Error>> {
    let mut capabilities = academic_rpc::handshake::PHASE1_CAPABILITY_IDS.to_vec();
    capabilities.extend_from_slice(academic_rpc::details::DETAILS_CAPABILITIES);
    capabilities.extend([
        academic_rpc::domain_details::CAPABILITY,
        ENCRYPTED_SYNTHETIC_POSTURE_CAPABILITY,
    ]);
    let response = negotiate_handshake(
        &client(1, 7, &capabilities),
        &ServerHandshakeConfig::encrypted_synthetic_scaffold(),
    )?;
    assert_eq!(
        response.capability_ids,
        [ENCRYPTED_SYNTHETIC_POSTURE_CAPABILITY]
    );
    assert_eq!(response.write_disposition, 16);
    assert_eq!(
        response.write_denial_reason,
        "ENCRYPTED_SYNTHETIC_SERVICE_UNAVAILABLE"
    );
    assert_eq!(response.lock_state, ProfileLockState::Locked as i32);
    assert!(response.projections.is_empty());
    assert_eq!(
        response
            .storage_schema
            .as_ref()
            .map(|schema| (schema.number, schema.semantic_version.as_str())),
        Some((2, "2.0.0"))
    );
    assert_eq!(response.vault_read_formats, ["AEAD_CHUNKED_V2"]);
    assert_eq!(response.vault_write_format, "AEAD_CHUNKED_V2");
    let policy = response.policy.as_ref().ok_or("missing posture")?;
    assert!(!policy.production_data_allowed);
    assert_eq!(policy.product_network, "NONE");
    assert!(policy.admission_receipt_digest.is_empty());
    assert!(policy.admission_platforms.is_empty());
    assert_eq!(
        policy.canonical_json,
        academic_admission::Posture::encrypted_synthetic().canonical_json_bytes()
    );
    assert!(matches!(
        authorize_mutable_request(&response, &ingest_request()),
        Err(RpcError::WriteDenied {
            disposition: 16,
            ..
        })
    ));
    let envelope = server_envelope(response);
    let bytes = encode_envelope_frame(&envelope, FrameClass::Handshake)?;
    assert_eq!(
        decode_envelope_frame(&bytes, FrameClass::Handshake)?,
        envelope
    );
    Ok(())
}

#[test]
fn encrypted_scaffold_requires_explicit_support_and_closed_configuration() {
    let config = ServerHandshakeConfig::encrypted_synthetic_scaffold();
    for hello in [
        client(1, 0, &[]),
        client(1, 0, &[academic_rpc::domain_details::CAPABILITY]),
        client(2, 0, &[ENCRYPTED_SYNTHETIC_POSTURE_CAPABILITY]),
        client(
            1,
            0,
            &[ENCRYPTED_SYNTHETIC_POSTURE_CAPABILITY, "unknown.future.v1"],
        ),
    ] {
        assert!(negotiate_handshake(&hello, &config).is_err());
    }
    let hello = client(1, 0, &[ENCRYPTED_SYNTHETIC_POSTURE_CAPABILITY]);
    for lock_state in [
        ProfileLockState::Unlocked,
        ProfileLockState::RepairRequired,
        ProfileLockState::Unspecified,
    ] {
        assert!(
            negotiate_handshake(
                &hello,
                &ServerHandshakeConfig {
                    lock_state,
                    ..config.clone()
                }
            )
            .is_err()
        );
    }
    let mut projected = config;
    projected
        .projections
        .push(academic_rpc::generated::ProjectionState::default());
    assert!(negotiate_handshake(&hello, &projected).is_err());
}

#[test]
fn opt_in_is_known_but_never_advertised_by_legacy_server() -> Result<(), Box<dyn std::error::Error>>
{
    let config = ServerHandshakeConfig::default();
    let old = client(1, 0, &[academic_rpc::domain_details::CAPABILITY]);
    let mut opted_in = old.clone();
    opted_in
        .capability_ids
        .push(ENCRYPTED_SYNTHETIC_POSTURE_CAPABILITY.to_owned());
    let original = negotiate_handshake(&old, &config)?;
    let current = negotiate_handshake(&opted_in, &config)?;
    assert_eq!(current, original);
    assert_eq!(
        encode_envelope_frame(&server_envelope(current), FrameClass::Handshake)?,
        encode_envelope_frame(&server_envelope(original), FrameClass::Handshake)?
    );
    assert!(
        !academic_rpc::handshake::PHASE1_CAPABILITY_IDS
            .contains(&ENCRYPTED_SYNTHETIC_POSTURE_CAPABILITY)
    );
    opted_in.capability_ids.push("unknown.future.v1".to_owned());
    assert_eq!(
        negotiate_handshake(&opted_in, &config)?.write_disposition,
        WriteDisposition::DeniedUnknownCapability as i32
    );
    Ok(())
}

#[test]
fn encrypted_wire_rejects_crossed_identity_authority_and_service_fields()
-> Result<(), Box<dyn std::error::Error>> {
    use prost::Message;
    let response = negotiate_handshake(
        &client(1, 0, &[ENCRYPTED_SYNTHETIC_POSTURE_CAPABILITY]),
        &ServerHandshakeConfig::encrypted_synthetic_scaffold(),
    )?;
    for case in 0..22 {
        let mut bad = response.clone();
        match case {
            0 => {
                bad.policy.as_mut().ok_or("policy")?.storage_mode =
                    "PLAINTEXT_TEMPORARY_SQLITE".to_owned()
            }
            1 => bad.policy.as_mut().ok_or("policy")?.storage_encryption = "NONE".to_owned(),
            2 => bad.policy.as_mut().ok_or("policy")?.object_format.clear(),
            3 => bad.policy.as_mut().ok_or("policy")?.production_data_allowed = true,
            4 => {
                bad.policy.as_mut().ok_or("policy")?.product_network =
                    "BROKERED_EGRESS_ONLY".to_owned()
            }
            5 => {
                bad.policy
                    .as_mut()
                    .ok_or("policy")?
                    .admission_receipt_digest = "a".repeat(64)
            }
            6 => bad
                .policy
                .as_mut()
                .ok_or("policy")?
                .admission_platforms
                .push("windows-x86_64".to_owned()),
            7 => bad
                .policy
                .as_mut()
                .ok_or("policy")?
                .canonical_json
                .push(b' '),
            8 => bad.storage_schema.as_mut().ok_or("schema")?.number = 1,
            9 => {
                bad.storage_schema
                    .as_mut()
                    .ok_or("schema")?
                    .semantic_version = "3.0.0".to_owned()
            }
            10 => bad.vault_read_formats = vec!["PLAINTEXT_SYNTHETIC_V1".to_owned()],
            11 => bad.vault_write_format = "PLAINTEXT_SYNTHETIC_V1".to_owned(),
            12 => bad.lock_state = ProfileLockState::Unlocked as i32,
            13 => bad.lock_state = ProfileLockState::RepairRequired as i32,
            14 => bad
                .projections
                .push(academic_rpc::generated::ProjectionState {
                    projection_id: "scope".to_owned(),
                    builder_schema_version: 1,
                    builder_digest: vec![1; 32],
                    ..Default::default()
                }),
            15 => bad.capability_ids.clear(),
            16 => bad
                .capability_ids
                .push(academic_rpc::domain_details::CAPABILITY.to_owned()),
            17 => bad.capability_ids.push("unknown.future.v1".to_owned()),
            18 => {
                bad.write_disposition = WriteDisposition::Allowed as i32;
                bad.write_denial_reason.clear();
            }
            19 => bad.write_denial_reason = "READY".to_owned(),
            20 => bad.negotiated_protocol_version = None,
            _ => bad.policy = None,
        }
        let envelope = server_envelope(bad);
        assert!(
            encode_envelope_frame(&envelope, FrameClass::Handshake).is_err(),
            "encoder accepted case {case}"
        );
        // Bypass the semantic encoder to exercise the decoder on hostile wire bytes.
        let payload = envelope.encode_to_vec();
        let frame = academic_rpc::frame::encode_frame(&payload, FrameClass::Handshake)?;
        assert!(
            decode_envelope_frame(&frame, FrameClass::Handshake).is_err(),
            "decoder accepted case {case}"
        );
    }
    let original = negotiate_handshake(&client(1, 0, &[]), &ServerHandshakeConfig::default())?;
    let mut token = original.clone();
    token
        .capability_ids
        .push(ENCRYPTED_SYNTHETIC_POSTURE_CAPABILITY.to_owned());
    assert!(encode_envelope_frame(&server_envelope(token), FrameClass::Handshake).is_err());
    let mut disposition = original;
    disposition.write_disposition = 16;
    disposition.write_denial_reason = "ENCRYPTED_SYNTHETIC_SERVICE_UNAVAILABLE".to_owned();
    assert!(encode_envelope_frame(&server_envelope(disposition), FrameClass::Handshake).is_err());
    Ok(())
}
