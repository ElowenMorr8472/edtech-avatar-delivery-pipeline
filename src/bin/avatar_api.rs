use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use edtech_avatar_pipeline::{
    avatar_service::{AvatarError, AvatarService, AvatarSubmission, EducatorReport},
    infrai_client::{ClientError, InfraiClient},
};
use serde::Serialize;

#[tokio::main]
async fn main() {
    let service = AvatarService::new(InfraiClient::from_env().expect("INFRAI_API_KEY must be set"));
    let app = Router::new()
        .route("/avatars", post(process_avatar))
        .with_state(service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("bind service");
    axum::serve(listener, app).await.expect("serve requests");
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

async fn process_avatar(
    State(service): State<AvatarService>,
    Json(input): Json<AvatarSubmission>,
) -> Result<Json<EducatorReport>, (StatusCode, Json<ErrorBody>)> {
    service.process(input).await.map(Json).map_err(map_error)
}

fn map_error(error: AvatarError) -> (StatusCode, Json<ErrorBody>) {
    let status = match &error {
        AvatarError::Invalid(_) => StatusCode::BAD_REQUEST,
        AvatarError::Infrai(ClientError::Rejected { status, .. }) => {
            StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_REQUEST)
        }
        AvatarError::Infrai(_) => StatusCode::BAD_GATEWAY,
    };
    (
        status,
        Json(ErrorBody {
            error: error.to_string(),
        }),
    )
}
