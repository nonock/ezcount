use super::*;

fn status(error: ApiError) -> StatusCode {
    error.into_response().status()
}

async fn body(error: ApiError) -> String {
    let bytes = axum::body::to_bytes(error.into_response().into_body(), 1024)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[test]
fn each_refusal_has_its_status() {
    assert_eq!(status(ApiError::BadRequest("no")), StatusCode::BAD_REQUEST);
    assert_eq!(status(ApiError::Unauthorized), StatusCode::UNAUTHORIZED);
    assert_eq!(status(ApiError::NotFound), StatusCode::NOT_FOUND);
    assert_eq!(status(ApiError::Conflict("taken")), StatusCode::CONFLICT);
    assert_eq!(status(ApiError::BadLogin), StatusCode::UNAUTHORIZED);
    assert_eq!(
        status(ApiError::TooManyAttempts),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        status(ApiError::TooManyRequests),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(status(ApiError::TooLarge), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        status(ApiError::StorageFull),
        StatusCode::INSUFFICIENT_STORAGE
    );
    assert_eq!(status(ApiError::NoRate), StatusCode::NOT_FOUND);
    assert_eq!(status(ApiError::LinkGone), StatusCode::GONE);
    assert_eq!(status(ApiError::DocumentGone), StatusCode::GONE);
    assert_eq!(
        status(ApiError::UpdateRequired),
        StatusCode::UPGRADE_REQUIRED
    );
    assert_eq!(
        status(ApiError::Unavailable("down".to_string())),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        status(ApiError::Internal("oops".to_string())),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn a_refusal_says_why() {
    assert_eq!(
        body(ApiError::BadRequest("invalid group id")).await,
        "invalid group id"
    );
    assert_eq!(
        body(ApiError::Conflict("username taken")).await,
        "username taken"
    );
}

#[tokio::test]
async fn what_went_wrong_inside_stays_inside() {
    let database: ApiError = rusqlite::Error::QueryReturnedNoRows.into();
    assert!(matches!(database, ApiError::Internal(_)));
    assert_eq!(
        body(ApiError::Internal("no such table: accounts".to_string())).await,
        "internal error"
    );
    assert_eq!(
        body(ApiError::Unavailable(
            "rates.example.com timed out".to_string()
        ))
        .await,
        "try later"
    );
}
