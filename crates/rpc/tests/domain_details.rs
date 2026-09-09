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

// These are codec-shaped DTOs, not accepted canonical sources or Project producers.
fn project_with_files(files: Vec<Value>) -> Value {
    let mut value = projection();
    let missing = json!({"state":"unavailable","reason":"PRODUCER_NOT_CONNECTED","source_ids":[]});
    value["projection"]["result"] = json!({"kind":"detail","detail":{
        "kind":"project","subject":{"kind":"project","domain_id":DOMAIN,"scope_id":SCOPE,"entity_id":ENTITY},
        "title":missing,"goals":missing,"analyzed_snapshot":missing,"current_snapshot":missing,
        "relation_groups":[],"files":{"state":"available","value":files,"provenance_refs":["event"]},
        "analyze":{"state":"unavailable","reason":"ANALYSIS_DISPATCH_NOT_CONNECTED"},
        "provider_preview":{"state":"unavailable","reason":"POLICY_STAGING_NOT_CONNECTED"}
    }});
    value
}

fn source_file(index: usize, file_length: u64, start: u64, end: u64, bytes: Vec<u8>) -> Value {
    json!({"path":format!("file-{index}.txt"),"content_digest":"a".repeat(64),
        "byte_length":file_length.to_string(),
        "source_excerpt":{"state":"available","provenance_refs":["event"],
            "value":{"start":start,"end":end,"bytes":bytes}}})
}

fn full_file_excerpts(count: usize) -> Vec<Value> {
    (0..count)
        .map(|index| source_file(index, 4096, 0, 4096, vec![0; 4096]))
        .collect()
}

fn add_text_excerpts(value: &mut Value, texts: &[String]) {
    value["projection"]["provenance"][1]["origin"]["evidence"] = json!(texts.iter().map(|text| {
        json!({"evidence_id":ENTITY,"artifact_id":EVENT,"representation_index":0,
            "locator":{"kind":"TEXT_BYTES","source_digest":"a".repeat(64),"start":0,"end":text.len()},
            "excerpt_digest":"a".repeat(64),"role":"SUPPORTS","strength":"DIRECT",
            "extraction_method":"codec-only","extractor_version":"1",
            "excerpt":{"state":"available","value":{"text":text,"encoding":"UTF-8"},"provenance_refs":["event"]}})
    }).collect::<Vec<_>>());
}

fn assert_project_validation(value: Value, valid: bool) -> TestResult {
    let typed: DomainReadReply = serde_json::from_value(value)?;
    // Generic framing must succeed: the domain contract, rather than another
    // frame/list/string bound, must discriminate these cases.
    let frame = encode(&typed)?;
    assert!(frame.len() < academic_rpc::details::MAX_DETAIL_BYTES);
    let reply: DomainReadReply = decode(&frame)?;
    assert_eq!(reply, typed);
    let DomainReadReply::Ready { projection, .. } = &reply else {
        return Err("expected ready-shaped codec DTO".into());
    };
    let request: DomainReadRequest = serde_json::from_value(json!({
        "command":"details_domain_read_v3","context":{"domain_id":DOMAIN,"scope_id":SCOPE},
        "selector":{"view":"domain_detail_v3","known_at_accept_seq":7,"valid_at_ms":50},
        "query":{"kind":"detail","subject":{"kind":"project","domain_id":DOMAIN,"scope_id":SCOPE,"entity_id":ENTITY}}
    }))?;
    request.validate()?;
    let frame_reply: DetailFrameReply = decode(&encode(&DetailFrameReply::Domain(reply.clone()))?)?;
    // Evaluate each public entry point even if an earlier one disagrees.
    let outcomes = [
        projection.validate().is_ok(),
        reply.validate().is_ok(),
        reply.validate_for(&request).is_ok(),
        frame_reply.validate().is_ok(),
        DomainReadReply::ready(projection.as_ref().clone()).is_ok(),
    ];
    assert_eq!(
        outcomes, [valid; 5],
        "projection/reply/request-bound/frame/ready validation"
    );
    Ok(())
}

#[test]
fn source_excerpt_valid_bytes_and_file_boundaries_are_lossless() -> TestResult {
    assert_project_validation(
        project_with_files(vec![
            source_file(0, 4096, 0, 4096, vec![0; 4096]),
            source_file(1, 4096, 4092, 4096, vec![0, 127, 128, 255]),
            source_file(2, 10, 3, 5, vec![255, 0]),
        ]),
        true,
    )
}

#[test]
fn source_excerpt_binary_total_accepts_exact_262144_bytes() -> TestResult {
    assert_eq!(MAX_EXCERPT_BYTES, 262_144);
    assert_project_validation(project_with_files(full_file_excerpts(64)), true)
}

#[test]
fn source_excerpt_binary_total_refuses_262145_bytes() -> TestResult {
    let mut files = full_file_excerpts(64);
    files.push(source_file(64, 1, 0, 1, vec![0]));
    assert_project_validation(project_with_files(files), false)
}

#[test]
fn source_excerpt_binary_total_refuses_review_65_file_case() -> TestResult {
    assert_project_validation(project_with_files(full_file_excerpts(65)), false)
}

#[test]
fn source_excerpt_mixed_utf8_and_binary_total_accepts_exact_262144_bytes() -> TestResult {
    let mut value = project_with_files(full_file_excerpts(63));
    let texts = ["é".repeat(1024), "한".repeat(682) + "ab"];
    assert_eq!(texts.iter().map(String::len).sum::<usize>(), 4096);
    add_text_excerpts(&mut value, &texts);
    assert_project_validation(value, true)
}

#[test]
fn source_excerpt_mixed_utf8_and_binary_total_refuses_262145_bytes() -> TestResult {
    let mut value = project_with_files(full_file_excerpts(63));
    add_text_excerpts(&mut value, &["é".repeat(1024), "한".repeat(682) + "abc"]);
    assert_project_validation(value, false)
}

#[test]
fn source_excerpt_text_only_budget_keeps_its_exact_boundary() -> TestResult {
    let texts = vec!["a".repeat(65_536); 4];
    let mut value = project_with_files(vec![source_file(0, 0, 0, 0, vec![])]);
    add_text_excerpts(&mut value, &texts);
    assert_project_validation(value.clone(), true)?;
    let mut over = texts;
    over.push("a".to_owned());
    add_text_excerpts(&mut value, &over);
    assert_project_validation(value, false)
}

#[test]
fn source_excerpt_reversed_range_is_refused() -> TestResult {
    assert_project_validation(
        project_with_files(vec![source_file(0, 4096, 9, 8, vec![0])]),
        false,
    )
}

#[test]
fn source_excerpt_length_mismatch_is_refused() -> TestResult {
    assert_project_validation(
        project_with_files(vec![source_file(0, 4096, 0, 2, vec![0])]),
        false,
    )
}

#[test]
fn source_excerpt_past_file_end_is_refused() -> TestResult {
    assert_project_validation(
        project_with_files(vec![source_file(0, 4096, 4095, 4097, vec![0; 2])]),
        false,
    )
}

#[test]
fn source_excerpt_empty_ranges_remain_valid_when_contained() -> TestResult {
    assert_project_validation(
        project_with_files(vec![
            source_file(0, 0, 0, 0, vec![]),
            source_file(1, 4096, 0, 0, vec![]),
            source_file(2, 4096, 17, 17, vec![]),
            source_file(3, 4096, 4096, 4096, vec![]),
        ]),
        true,
    )
}

#[test]
fn source_excerpt_empty_range_beyond_eof_is_refused() -> TestResult {
    assert_project_validation(
        project_with_files(vec![source_file(0, 4096, 4097, 4097, vec![])]),
        false,
    )
}

#[test]
fn source_excerpt_empty_range_with_bytes_is_refused() -> TestResult {
    assert_project_validation(
        project_with_files(vec![source_file(0, 4096, 0, 0, vec![0])]),
        false,
    )
}

#[test]
fn source_excerpt_exact_integer_bounds_and_unavailable_sources_are_preserved() -> TestResult {
    let safe = academic_rpc::details::MAX_SAFE_INTEGER;
    let mut value = project_with_files(vec![source_file(0, u64::MAX, safe - 1, safe, vec![0])]);
    assert_project_validation(value.clone(), true)?;
    value["projection"]["result"]["detail"]["files"]["value"][0]["byte_length"] =
        json!((safe - 1).to_string());
    assert_project_validation(value.clone(), false)?;
    value["projection"]["result"]["detail"]["files"]["value"][0]["source_excerpt"] = json!({
        "state":"unavailable","reason":"SOURCE_BODY_UNAVAILABLE","source_ids":[]
    });
    assert_project_validation(value, true)?;
    let unsafe_number = project_with_files(vec![source_file(0, u64::MAX, safe, safe + 1, vec![0])]);
    let reply: DomainReadReply = serde_json::from_value(unsafe_number)?;
    assert!(encode(&reply).is_err());
    assert!(reply.validate().is_err());
    Ok(())
}

#[test]
fn source_excerpt_validation_keeps_provenance_and_context_checks() -> TestResult {
    let value = project_with_files(vec![source_file(0, 1, 0, 1, vec![0])]);
    assert_project_validation(value.clone(), true)?;
    let mut missing_ref = value.clone();
    missing_ref["projection"]["result"]["detail"]["files"]["value"][0]["source_excerpt"]["provenance_refs"] =
        json!(["missing"]);
    assert_project_validation(missing_ref, false)?;
    let mut wrong_scope = value;
    wrong_scope["projection"]["result"]["detail"]["subject"]["scope_id"] = json!(ENTITY);
    assert_project_validation(wrong_scope, false)
}
