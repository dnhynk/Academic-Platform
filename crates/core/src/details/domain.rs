//! Plaintext acquisition adapter for the shared authenticated domain projector.
use super::{DetailContext, profile_identity};
use crate::{
    authenticated_acceptance::VaultAccess,
    domain_read::{authenticate, project, query_failure},
    service::AcceptanceService,
};
use academic_domain::TimestampMillis;
use academic_rpc::domain_details as dto;
use academic_store::{
    profile::SyntheticProfile,
    queries::{DomainHistoryRequest, domain_history_snapshot},
};

type ReadResult<T> = Result<T, dto::ReadFailure>;

impl DetailContext {
    /// Reads through the same host-selected profile and its existing authority.
    /// This operation has no decision, importer, source-body or actor parameter.
    pub fn read_domain(
        &self,
        profile: &SyntheticProfile,
        service: &AcceptanceService,
        request: &dto::DomainReadRequest,
        now: TimestampMillis,
    ) -> dto::DomainReadReply {
        match self.domain_projection(profile, service, request, now) {
            Ok(projection) => dto::DomainReadReply::ready(projection).unwrap_or_else(|_| {
                dto::DomainReadReply::unavailable(dto::ReadFailure::ResultTooLarge)
            }),
            Err(reason) => dto::DomainReadReply::unavailable(reason),
        }
    }

    fn domain_projection(
        &self,
        profile: &SyntheticProfile,
        service: &AcceptanceService,
        request: &dto::DomainReadRequest,
        now: TimestampMillis,
    ) -> ReadResult<dto::DomainProjection> {
        let (context, selector, query) = request.parts();
        if let dto::Query::Detail { subject } = query
            && subject.context() != *context
        {
            return Err(dto::ReadFailure::ContextMismatch);
        }
        request
            .validate()
            .map_err(|_| dto::ReadFailure::SelectorUnavailable)?;
        if !service.uses_profile(profile)
            || profile_identity(profile).map_err(|_| dto::ReadFailure::ProfileUnavailable)?
                != self.profile_id
        {
            return Err(dto::ReadFailure::ProfileMismatch);
        }
        let valid = match selector.valid_at_ms {
            Some(value) => value,
            None => {
                u64::try_from(now.value()).map_err(|_| dto::ReadFailure::SelectorUnavailable)?
            }
        };
        if valid > academic_rpc::details::MAX_SAFE_INTEGER {
            return Err(dto::ReadFailure::SelectorUnavailable);
        }
        let source = domain_history_snapshot(
            &mut profile
                .open_reader()
                .map_err(|_| dto::ReadFailure::ProfileUnavailable)?,
            &DomainHistoryRequest {
                domain_id: context.domain_id,
                scope_id: context.scope_id,
                known_at_accept_seq: selector.known_at_accept_seq,
                valid_at: TimestampMillis::new(
                    i64::try_from(valid).map_err(|_| dto::ReadFailure::SelectorUnavailable)?,
                ),
            },
        )
        .map_err(query_failure)?;
        let snapshot = authenticate(source, &self.trust)?;
        if profile_identity(profile).map_err(|_| dto::ReadFailure::ProfileUnavailable)?
            != self.profile_id
        {
            return Err(dto::ReadFailure::ProfileMismatch);
        }
        let result = project(
            &snapshot,
            VaultAccess::Plain(service.vault()),
            &self.profile_id,
            query,
        )?;
        if profile_identity(profile).map_err(|_| dto::ReadFailure::ProfileUnavailable)?
            != self.profile_id
        {
            return Err(dto::ReadFailure::ProfileMismatch);
        }
        Ok(result)
    }
}
