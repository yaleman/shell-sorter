use std::time::{SystemTime, UNIX_EPOCH};

use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};

/// Middleware to add no-cache headers to prevent browser caching
pub(crate) async fn no_cache_middleware(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;

    // Get the headers map mutably
    let headers = response.headers_mut();

    // Add no-cache headers for all responses
    headers.insert(
        "Cache-Control",
        HeaderValue::from_static("no-cache, no-store, must-revalidate, max-age=0"),
    );
    headers.insert("Pragma", HeaderValue::from_static("no-cache"));
    headers.insert("Expires", HeaderValue::from_static("0"));

    // Generate ETag with current timestamp
    if let Ok(timestamp) = SystemTime::now().duration_since(UNIX_EPOCH) {
        let etag_value = format!("\"{}\"", timestamp.as_secs());
        if let Ok(etag_header) = HeaderValue::from_str(&etag_value) {
            headers.insert("ETag", etag_header);
        }
    }

    // Additional headers for static files (JS, CSS, HTML)
    if path.ends_with(".js")
        || path.ends_with(".css")
        || path.ends_with(".html")
        || path.ends_with(".htm")
    {
        headers.insert(
            "Cache-Control",
            HeaderValue::from_static("no-cache, no-store, must-revalidate, max-age=0, private"),
        );
    }

    response
}
