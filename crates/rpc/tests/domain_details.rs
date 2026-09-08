use academic_rpc::{
    FrameClass, decode_envelope_frame,
    details::{decode, encode},
    domain_details::wire::*,
    domain_details::*,
    encode_envelope_frame,
    generated::{DetailRequestFrame, LocalCoreEnvelope, local_core_envelope::Payload},
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const DOMAIN: &str = "01900000-0000-7000-8000-000000000001";
const SCOPE: &str = "01900000-0000-7000-8000-000000000002";
const ENTITY: &str = "01900000-0000-7000-8000-000000000003";
const CLAIM: &str = "01900000-0000-7000-8000-000000000004";
const EVENT: &str = "01900000-0000-7000-8000-000000000005";

fn request() -> Result<DomainReadRequest, Box<dyn std::error::Error>> {
    Ok(serde_json::from_value(
        json!({"command":"details_domain_read_v3",
        "context":{"domain_id":DOMAIN,"scope_id":SCOPE},
        "selector":{"view":"domain_detail_v3","known_at_accept_seq":7,"valid_at_ms":50},
        "query":{"kind":"index","surface":"concept"}}),
    )?)
}

fn projection() -> Value {
    let digest = "a".repeat(64);
    let origin = json!({"event_id":EVENT,"origin_seq":"18446744073709551615","origin_observed_at_ms":"-9223372036854775808",
        "actor":{"kind":"IMPORTER","name":"synthetic","version":"1"},"domain_id":DOMAIN,
        "accepted_batch_envelope_digest":digest,"accepted_batch_payload_digest":digest});
    json!({"version":1,"state":"ready","schema_version":3,"projection":{
        "source":{"kind":"domain_projection","projector_version":PROJECTOR_VERSION},
        "binding":{"profile_id":"selected-profile","revision":2,"domain_id":DOMAIN,"scope_id":SCOPE,"known_at_accept_seq":7,"valid_at_ms":50,"source_outbox_seq":2,"source_ledger_digest":digest},
        "read_sources":[{"ref":"canonical","authority":{"kind":"CANONICAL_SNAPSHOT",
            "coordinates":{"domain_id":DOMAIN,"known_at_accept_seq":7,"valid_at_ms":50,"source_outbox_seq":2,"source_ledger_digest":digest},
            "snapshot_adapter_version":"signed-source-v3","aggregate_source_row_digest":null,
            "policy_registry":{"resolver_version":"canonical-resolver","policy_registry_version":"registry-v3","policy_registry_hash":digest,
                "predicate_policies":[{"predicate_id":"entity.label","policy":"USER_OWNED"}]}}}],
        "provenance":[
            {"ref":"event","source_ref":"canonical","origin":{"kind":"ACCEPTED_EVENT","event":origin,"accept_seq":7,"scope_id":SCOPE}},
            {"ref":"claim","source_ref":"canonical","origin":{"kind":"ACCEPTED_CLAIM","claim_id":CLAIM,"domain_id":DOMAIN,"scope_id":SCOPE,"authority_class":"DIRECT_OBSERVATION","epistemic_status":"CODE_OBSERVED",
                "origin_event":{"state":"available","value":origin,"provenance_refs":["event"]},"accept_seq":7,"valid_from_ms":"-10","valid_to_ms":null,"evidence":[],"resolution_policy":"USER_OWNED","graph_row":null}},
            {"ref":"index","source_ref":"canonical","origin":{"kind":"VERIFIED_QUERY","query":{
                "context":{"domain_id":DOMAIN,"scope_id":SCOPE},"coordinates":{"known_at_accept_seq":7,"valid_at_ms":50},
                "selection":{"kind":"INDEX","surface":"concept","order":"CANONICAL_ID_ASC","max_entries":256},"completeness":"COMPLETE","returned_count":1}}}],
        "result":{"kind":"index","surface":"concept","entries":[{"subject":{"kind":"concept","domain_id":DOMAIN,"scope_id":SCOPE,"entity_id":ENTITY},
            "title":{"state":"available","value":"  CONFIRMED\nreported  ","provenance_refs":["claim"]},"source_ids":[{"kind":"ENTITY","value":ENTITY}]}]}}})
}

fn checked(value: Value) -> Result<DomainReadReply, Box<dyn std::error::Error>> {
    let typed: DomainReadReply = serde_json::from_value(value)?;
    let reply: DomainReadReply = decode(&encode(&typed)?)?;
    reply.validate()?;
    Ok(reply)
}

#[test]
fn domain_command_is_closed_and_legacy_imported_bytes_are_unchanged() -> TestResult {
    let request = request()?;
    let wrapped = DetailFrameRequest::Domain(request.clone());
    assert_eq!(encode(&wrapped)?, encode(&request)?);
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
    let old = academic_rpc::details::DetailRequest::DetailsRead {
        selector: academic_rpc::details::DetailSelector::default(),
    };
    assert_eq!(encode(&old)?, encode(&DetailFrameRequest::Imported(old))?);
    let mut malformed = serde_json::to_value(&request)?;
    malformed["actor"] = json!({"kind":"USER","user_id":ENTITY});
    assert!(decode::<DetailFrameRequest>(&encode(&malformed)?).is_err());
    malformed.as_object_mut().ok_or("object")?.remove("actor");
    malformed["query"] = json!({"kind":"reject","claim_id":CLAIM});
    assert!(decode::<DetailFrameRequest>(&encode(&malformed)?).is_err());
    Ok(())
}

#[test]
fn verified_projection_preserves_reported_text_and_independent_authority() -> TestResult {
    let reply = checked(projection())?;
    reply.validate_for(&request()?)?;
    assert!(!format!("{reply:?}").contains("reported"));
    let value = serde_json::to_value(&reply)?;
    assert_eq!(
        value["projection"]["result"]["entries"][0]["title"]["value"],
        "  CONFIRMED\nreported  "
    );
    assert_eq!(
        value["projection"]["provenance"][1]["origin"]["authority_class"],
        "DIRECT_OBSERVATION"
    );
    assert_eq!(
        value["projection"]["provenance"][1]["origin"]["epistemic_status"],
        "CODE_OBSERVED"
    );
    assert!(value.get("receipt_decision").is_none());
    assert!(value.get("receipt_id").is_none());
    assert!(decode::<academic_rpc::details::DetailReply>(&encode(&reply)?).is_err());
    Ok(())
}

#[test]
fn closure_cycles_missing_references_and_mixed_coordinates_are_refused() -> TestResult {
    for pointer in [
        "/projection/binding/scope_id",
        "/projection/read_sources/0/authority/coordinates/domain_id",
        "/projection/provenance/1/origin/scope_id",
    ] {
        let mut value = projection();
        *value.pointer_mut(pointer).ok_or("pointer")? = json!(ENTITY);
        assert!(checked(value).is_err());
    }
    for bad_ref in ["missing", "claim"] {
        let mut value = projection();
        value["projection"]["provenance"][1]["origin"]["origin_event"]["provenance_refs"] =
            json!([bad_ref]);
        assert!(checked(value).is_err());
    }
    let mut mismatch = request()?;
    let DomainReadRequest::DetailsDomainReadV3 { selector, .. } = &mut mismatch;
    selector.known_at_accept_seq = Some(6);
    assert!(checked(projection())?.validate_for(&mismatch).is_err());
    let mut rounded = projection();
    rounded["projection"]["binding"]["known_at_accept_seq"] = json!(9_007_199_254_740_992_u64);
    assert!(checked(rounded).is_err());
    Ok(())
}

#[test]
fn complete_empty_index_requires_the_same_scoped_query_witness() -> TestResult {
    let mut value = projection();
    value["projection"]["result"]["entries"] = json!([]);
    assert!(checked(value.clone()).is_err());
    value["projection"]["provenance"][2]["origin"]["query"]["returned_count"] = json!(0);
    checked(value.clone())?;
    value["projection"]["provenance"]
        .as_array_mut()
        .ok_or("array")?
        .pop();
    assert!(checked(value).is_err());
    Ok(())
}

#[test]
fn shared_provenance_ancestors_keep_bounded_depth_and_cycles_closed() -> TestResult {
    let mut value = projection();
    let template = value["projection"]["provenance"][1].clone();
    let mut previous = vec!["claim".to_owned()];
    for level in 1..24 {
        let mut current = Vec::new();
        for branch in ["a", "b"] {
            let reference = format!("layer-{level:02}-{branch}");
            let mut entry = template.clone();
            entry["ref"] = json!(reference);
            let mut refs = vec!["event".to_owned()];
            refs.extend(previous.iter().cloned());
            entry["origin"]["origin_event"]["provenance_refs"] = json!(refs);
            value["projection"]["provenance"]
                .as_array_mut()
                .ok_or("array")?
                .push(entry);
            current.push(reference);
        }
        previous = current;
    }
    checked(value.clone())?;
    let mut excessive = template.clone();
    excessive["ref"] = json!("too-deep");
    excessive["origin"]["origin_event"]["provenance_refs"] = json!(["event", previous[0]]);
    value["projection"]["provenance"]
        .as_array_mut()
        .ok_or("array")?
        .push(excessive);
    assert!(checked(value.clone()).is_err());
    value["projection"]["provenance"]
        .as_array_mut()
        .ok_or("array")?
        .pop();
    value["projection"]["provenance"][1]["origin"]["origin_event"]["provenance_refs"] =
        json!(["event", previous[1]]);
    assert!(checked(value).is_err());
    Ok(())
}

#[test]
fn question_statuses_and_distinct_original_id_kinds_are_lossless() -> TestResult {
    for status in [
        "OPEN",
        "PARTIALLY_RESOLVED",
        "RESOLVED",
        "REFRAMED",
        "OBSOLETE",
        "REOPENED",
    ] {
        let domain: academic_domain::question::QuestionStatus =
            serde_json::from_value(json!(status))?;
        let dto: QuestionStatus = serde_json::from_value(serde_json::to_value(domain)?)?;
        assert_eq!(serde_json::to_value(dto)?, json!(status));
    }
    let entity: OriginalId = serde_json::from_value(json!({"kind":"ENTITY","value":ENTITY}))?;
    let lecture: OriginalId =
        serde_json::from_value(json!({"kind":"LECTURE_SESSION","value":ENTITY}))?;
    let repository: OriginalId =
        serde_json::from_value(json!({"kind":"REPOSITORY","value":ENTITY}))?;
    assert_ne!(entity, lecture);
    assert_ne!(entity, repository);
    assert_ne!(lecture, repository);
    let raw_segment = OriginalId::DomainRawSegment {
        value: "segment:17".to_owned().try_into()?,
    };
    let raw_segment_bytes = br#"{"kind":"RAW_SEGMENT","value":"segment:17"}"#;
    assert_eq!(serde_json::to_vec(&raw_segment)?, raw_segment_bytes);
    assert_eq!(
        serde_json::from_slice::<OriginalId>(raw_segment_bytes)?,
        raw_segment
    );
    let implicit_renamed_tag = json!({"kind":"DOMAIN_RAW_SEGMENT","value":"segment:17"});
    assert!(serde_json::from_value::<OriginalId>(implicit_renamed_tag).is_err());
    for invalid in ["+1", "01", "-0", "18446744073709551616"] {
        assert!(serde_json::from_value::<U64Decimal>(json!(invalid)).is_err());
    }
    Ok(())
}

#[test]
fn relation_group_preserves_its_subject_and_selected_predicate() -> TestResult {
    let mut value = projection();
    let predicate = academic_domain::predicates::PredicateName::UsedIn
        .descriptor()
        .predicate_id;
    let subject = value["projection"]["result"]["entries"][0]["subject"].clone();
    let title = value["projection"]["result"]["entries"][0]["title"].clone();
    let missing = json!({"state":"unavailable","reason":"PRODUCER_NOT_CONNECTED","source_ids":[]});
    value["projection"]["provenance"][2]["origin"]["query"]["selection"] = json!({
        "kind":"RELATIONS","subject":subject,"predicate_ids":[predicate],"direction":"OUTGOING","order":"CLAIM_ID_ASC","max_entries":4096
    });
    value["projection"]["result"] = json!({"kind":"detail","detail":{
        "kind":"concept","subject":subject,"title":title,"state":missing,"freshness":missing,"last_strong_evidence":missing,
        "relation_groups":[{"kind":"USED_IN","relations":{"state":"available","provenance_refs":["index"],"value":[{
            "claim_id":CLAIM,"subject":subject,"predicate_id":predicate,"scope_id":SCOPE,
            "target":{"state":"available","value":{"kind":"ENTITY","value":ENTITY},"provenance_refs":["claim"]},
            "label":missing,"confidence":missing,"provenance_ref":"claim","user_decision":missing,
            "writes":{"state":"unavailable","reason":"CANONICAL_RELATION_WRITE_ADAPTER_MISSING"}
        }]}}]
    }});
    checked(value.clone())?;
    let relation = "/projection/result/detail/relation_groups/0/relations/value/0";
    let mut wrong_subject = value.clone();
    wrong_subject.pointer_mut(relation).ok_or("relation")?["subject"]["entity_id"] = json!(CLAIM);
    assert!(checked(wrong_subject).is_err());
    value.pointer_mut(relation).ok_or("relation")?["predicate_id"] = json!("entity.label");
    assert!(checked(value).is_err());
    Ok(())
}
