//! A remote admission is what the scoped-remote arm produced, and nothing else.
//!
//! `RemoteAdmission`'s fields are private and it has no public constructor, so
//! the only value of that type is one `SttPolicy::route_for` returned. An
//! `SttRoute::ScopedRemote` cannot be assembled around a forged one either.

use academic_model_run::{ModelVersion, ProviderId, RetentionDeclaration};
use academic_transcription::{RemoteAdmission, SttRoute};

fn forge(
    provider: ProviderId,
    model_version: ModelVersion,
    retention: RetentionDeclaration,
) -> RemoteAdmission {
    RemoteAdmission {
        provider,
        model_version,
        retention,
    }
}

fn forge_route(admission: RemoteAdmission) -> SttRoute {
    SttRoute::ScopedRemote { admission }
}

fn main() {}
