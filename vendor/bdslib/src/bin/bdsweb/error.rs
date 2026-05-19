use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppError {
    Rpc(String),
    Http(reqwest::Error),
    Json(serde_json::Error),
    Render(askama::Error),
    Msg(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Rpc(m)    => write!(f, "RPC error: {m}"),
            AppError::Http(e)   => write!(f, "HTTP error: {e}"),
            AppError::Json(e)   => write!(f, "JSON parse error: {e}"),
            AppError::Render(e) => write!(f, "Template error: {e}"),
            AppError::Msg(m)    => write!(f, "{m}"),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let msg = self.to_string();
        let body = format!(
            r#"<!doctype html><html lang="en">
<head><meta charset="UTF-8"><title>Error — bdsnode</title>
<script src="https://cdn.tailwindcss.com"></script></head>
<body class="bg-gray-950 text-red-400 p-10 font-mono">
<h1 class="text-2xl mb-4 text-red-300">Internal Error</h1>
<pre class="text-sm whitespace-pre-wrap">{msg}</pre>
<a href="/" class="mt-6 inline-block text-blue-400 underline">← back to dashboard</a>
</body></html>"#
        );
        (StatusCode::INTERNAL_SERVER_ERROR, Html(body)).into_response()
    }
}

impl From<reqwest::Error>    for AppError { fn from(e: reqwest::Error)    -> Self { AppError::Http(e) } }
impl From<serde_json::Error> for AppError { fn from(e: serde_json::Error) -> Self { AppError::Json(e) } }
impl From<askama::Error>     for AppError { fn from(e: askama::Error)     -> Self { AppError::Render(e) } }
