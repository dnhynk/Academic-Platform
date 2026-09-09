//! Additive commands inside the existing bounded JSON frames, without changing
//! any imported request, signed request digest or decision receipt encoding.
use super::{CAPABILITY, DomainReadReply, DomainReadRequest};
use crate::{
    RpcError,
    details::{DetailReply, DetailRequest},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DetailFrameRequest {
    Imported(DetailRequest),
    Domain(DomainReadRequest),
}

impl DetailFrameRequest {
    pub const fn capability(&self) -> &'static str {
        match self {
            Self::Imported(request) => request.capability(),
            Self::Domain(_) => CAPABILITY,
        }
    }
    pub fn validate(&self) -> Result<(), RpcError> {
        match self {
            Self::Imported(DetailRequest::DetailsDecide { decision }) => {
                crate::details::reference(&decision.relation_id)
            }
            Self::Imported(_) => Ok(()),
            Self::Domain(request) => request.validate(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DetailFrameReply {
    Imported(Box<DetailReply>),
    Domain(DomainReadReply),
}

impl DetailFrameReply {
    pub fn validate(&self) -> Result<(), RpcError> {
        match self {
            Self::Imported(reply) => reply.validate(),
            Self::Domain(reply) => reply.validate(),
        }
    }
}
