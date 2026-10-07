use crate::infrai_client::{ClientError, DirectoryUser, InfraiClient};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Deserialize)]
pub struct JoinRequest {
    pub company_domain: String,
    pub employee_email: String,
    pub ownership_txt: String,
    pub asset: AssetIngestion,
}

#[derive(Debug, Deserialize)]
pub struct AssetIngestion {
    pub asset_id: String,
    pub source_uri: String,
    pub creator_id: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct JoinOutcome {
    pub workspace: String,
    pub user_id: String,
    pub employee_email: String,
    pub membership: Membership,
    pub pipeline: MediaPipeline,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Membership {
    Joined,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct MediaPipeline {
    pub asset_id: String,
    pub ingestion: PipelineState,
    pub processing_job: PipelineState,
    pub creator_delivery: PipelineState,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineState {
    Accepted,
    Queued,
    AwaitingProcessing,
}

#[derive(Debug, Error)]
pub enum JoinError {
    #[error("employee email must belong to {domain}")]
    DomainMismatch { domain: String },
    #[error("domain ownership is not verified")]
    DomainNotVerified,
    #[error(transparent)]
    Infrai(#[from] ClientError),
}

pub fn email_belongs_to_domain(email: &str, domain: &str) -> bool {
    let normalized_domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    email
        .rsplit_once('@')
        .map(|(_, email_domain)| email_domain.eq_ignore_ascii_case(&normalized_domain))
        .unwrap_or(false)
}

pub async fn join_verified_employee(
    client: &InfraiClient,
    request: JoinRequest,
) -> Result<JoinOutcome, JoinError> {
    if !email_belongs_to_domain(&request.employee_email, &request.company_domain) {
        return Err(JoinError::DomainMismatch {
            domain: request.company_domain,
        });
    }

    let zone = client.add_domain(&request.company_domain).await?;
    client
        .upsert_ownership_txt(&zone.zone_id, &request.company_domain, &request.ownership_txt)
        .await?;
    let proof = client.verify_domain(&request.company_domain).await?;
    if !proof.verified {
        return Err(JoinError::DomainNotVerified);
    }

    let DirectoryUser { id, email, .. } = client.user_by_email(&request.employee_email).await?;
    Ok(JoinOutcome {
        workspace: request.company_domain,
        user_id: id,
        employee_email: email,
        membership: Membership::Joined,
        pipeline: MediaPipeline {
            asset_id: request.asset.asset_id,
            ingestion: PipelineState::Accepted,
            processing_job: PipelineState::Queued,
            creator_delivery: PipelineState::AwaitingProcessing,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::email_belongs_to_domain;

    #[test]
    fn only_exact_company_domain_can_join() {
        assert!(email_belongs_to_domain("editor@stream.example", "stream.example"));
        assert!(!email_belongs_to_domain("editor@contractor.example", "stream.example"));
        assert!(!email_belongs_to_domain("editor@fake-stream.example", "stream.example"));
    }
}

