use crate::infrai_client::{ClientError, InfraiClient};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Deserialize)]
pub struct AvatarSubmission {
    pub user_id: String,
    pub course_id: String,
    pub filename: String,
    pub image_bytes: Vec<u8>,
    pub submitted_at_epoch: u64,
    pub deadline_epoch: u64,
    pub operation_id: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Ready,
    Late,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct EducatorReport {
    pub user_id: String,
    pub course_id: String,
    pub avatar: String,
    pub delivery_status: DeliveryStatus,
}

#[derive(Debug, Error)]
pub enum AvatarError {
    #[error("invalid submission: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Infrai(#[from] ClientError),
}

#[derive(Clone)]
pub struct AvatarService {
    client: InfraiClient,
}

impl AvatarService {
    pub fn new(client: InfraiClient) -> Self {
        Self { client }
    }

    pub async fn process(&self, input: AvatarSubmission) -> Result<EducatorReport, AvatarError> {
        if input.image_bytes.is_empty() {
            return Err(AvatarError::Invalid("image is empty"));
        }
        if input.user_id.is_empty() || input.course_id.is_empty() || input.operation_id.is_empty() {
            return Err(AvatarError::Invalid(
                "user_id, course_id, and operation_id are required",
            ));
        }
        let uploaded = self
            .client
            .upload(
                input.image_bytes,
                &input.filename,
                &format!("{}-upload", input.operation_id),
            )
            .await?;
        let cropped = self
            .client
            .smart_crop(&uploaded, &format!("{}-crop", input.operation_id))
            .await?;
        let avatar = self
            .client
            .optimize(&cropped, &format!("{}-optimize", input.operation_id))
            .await?;
        self.client
            .update_user_avatar(
                &input.user_id,
                &avatar,
                &format!("{}-profile", input.operation_id),
            )
            .await?;
        Ok(build_report(
            input.user_id,
            input.course_id,
            avatar,
            input.submitted_at_epoch,
            input.deadline_epoch,
        ))
    }
}

fn build_report(
    user_id: String,
    course_id: String,
    avatar: String,
    submitted: u64,
    deadline: u64,
) -> EducatorReport {
    let delivery_status = if submitted <= deadline {
        DeliveryStatus::Ready
    } else {
        DeliveryStatus::Late
    };
    EducatorReport {
        user_id,
        course_id,
        avatar,
        delivery_status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submission_after_course_deadline_is_reported_late() {
        let report = build_report(
            "learner-42".into(),
            "rust-101".into(),
            "avatar-ref".into(),
            1_726_000_001,
            1_726_000_000,
        );
        assert_eq!(report.delivery_status, DeliveryStatus::Late);
        assert_eq!(report.course_id, "rust-101");
    }
}
