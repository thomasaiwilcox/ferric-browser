use super::{ByteRangeResponse, FixtureRequest, FixtureResponse};

#[derive(Default)]
pub(super) struct SessionState {
    authenticated: bool,
    editor_download_armed: bool,
    document_gets: u32,
    form_posts: u32,
}

// Keep the fixed fixture catalogue together so a route's method, response and
// synthetic trust boundary are visible in one place.
#[allow(clippy::too_many_lines)]
pub(super) fn route(
    request: &FixtureRequest,
    path: &str,
    session: &mut SessionState,
) -> Option<FixtureResponse> {
    let response = match (request.method.as_str(), path) {
        ("GET", "/versioning/") => html(include_str!(
            "../../../ferric-browser-engine-qt/tests/fixtures/versioning/index.html"
        )),
        ("GET", "/versioning/document") => {
            session.document_gets = session.document_gets.saturating_add(1);
            html(include_str!(
                "../../../ferric-browser-engine-qt/tests/fixtures/versioning/document.html"
            ))
        }
        ("GET", "/versioning/document-request-count") => {
            text(200, "OK", &session.document_gets.to_string())
        }
        ("GET", "/versioning/form") => html(include_str!(
            "../../../ferric-browser-engine-qt/tests/fixtures/versioning/form.html"
        )),
        ("GET", "/versioning/form-auto-post") => html(
            "<!doctype html><meta charset=\"utf-8\"><title>Synthetic auto POST</title><form method=\"post\" action=\"/versioning/form/result\"><input name=\"synthetic\" value=\"1\"></form><script>addEventListener('load', () => setTimeout(() => document.forms[0].submit(), 50))</script>",
        ),
        ("POST", "/versioning/form/result") if !request.body.is_empty() => {
            session.form_posts = session.form_posts.saturating_add(1);
            html(
                "<!doctype html><meta charset=\"utf-8\"><title>Versioning form result</title><h1>Synthetic POST result</h1><p id=\"result\">accepted without replay</p>",
            )
        }
        ("GET", "/versioning/form-post-count") => text(200, "OK", &session.form_posts.to_string()),
        ("GET", "/versioning/redirect-303") => html(
            "<!doctype html><meta charset=\"utf-8\"><title>Versioning 303 form</title><form method=\"post\" action=\"/versioning/redirect-303/submit\"><input name=\"redirect\" value=\"synthetic\"><button type=\"submit\">Submit</button></form>",
        ),
        ("POST", "/versioning/redirect-303/submit") if request.body == b"redirect=synthetic" => {
            super::redirect_response(303, "/versioning/redirect-303/result")
        }
        ("GET", "/versioning/redirect-303/result") => html(
            "<!doctype html><meta charset=\"utf-8\"><title>Versioning 303 result</title><p id=\"result\">safe redirected GET</p>",
        ),
        ("GET", "/versioning/redirect-307") => html(
            "<!doctype html><meta charset=\"utf-8\"><title>Versioning 307 form</title><form method=\"post\" action=\"/versioning/redirect-307/submit\"><input name=\"redirect\" value=\"synthetic\"><button type=\"submit\">Submit</button></form>",
        ),
        ("POST", "/versioning/redirect-307/submit") if request.body == b"redirect=synthetic" => {
            super::redirect_response(307, "/versioning/redirect-307/result")
        }
        ("POST", "/versioning/redirect-307/result") if request.body == b"redirect=synthetic" => {
            html(
                "<!doctype html><meta charset=\"utf-8\"><title>Versioning 307 result</title><p id=\"result\">POST preserved</p>",
            )
        }
        ("GET", "/versioning/controlled") => html(include_str!(
            "../../../ferric-browser-engine-qt/tests/fixtures/versioning/controlled.html"
        )),
        ("GET", "/versioning/media") => html(include_str!(
            "../../../ferric-browser-engine-qt/tests/fixtures/versioning/media.html"
        )),
        ("GET", "/versioning/media-silence.wav") => media_silence_wav(request),
        ("GET", "/versioning/editor-refill") => html(include_str!(
            "../../../ferric-browser-engine-qt/tests/fixtures/versioning/editor_refill.html"
        )),
        ("GET", "/versioning/editor") => {
            let editor = include_str!(
                "../../../ferric-browser-engine-qt/tests/fixtures/versioning/editor.html"
            );
            if session.editor_download_armed {
                session.editor_download_armed = false;
                html(&format!(
                    "{editor}<script>window.addEventListener('load', () => setTimeout(() => location.assign('/versioning/slow-download?armed=1'), 400));</script>"
                ))
            } else {
                html(editor)
            }
        }
        ("POST", "/versioning/editor/arm-download") => {
            session.editor_download_armed = true;
            text(200, "OK", "next editor load armed")
        }
        ("POST", "/versioning/editor/autosave") if !request.body.is_empty() => {
            text(200, "OK", "synthetic autosave received")
        }
        ("GET", "/versioning/lifecycle") => html(include_str!(
            "../../../ferric-browser-engine-qt/tests/fixtures/versioning/lifecycle.html"
        )),
        ("GET", "/versioning/mutation") => html(include_str!(
            "../../../ferric-browser-engine-qt/tests/fixtures/versioning/mutation.html"
        )),
        ("GET", "/versioning/frames") => {
            let origin = alternate_loopback_origin(request);
            html(
                &include_str!(
                    "../../../ferric-browser-engine-qt/tests/fixtures/versioning/frames.html"
                )
                .replace(
                    "{{CROSS_ORIGIN}}",
                    &format!("{origin}/versioning/frame-child?cross=1"),
                ),
            )
        }
        ("GET", "/versioning/frame-child") => html(include_str!(
            "../../../ferric-browser-engine-qt/tests/fixtures/versioning/frame-child.html"
        )),
        ("GET", "/versioning/frame-branch") => {
            let origin = alternate_loopback_origin(request);
            html(&format!(
                r#"<!doctype html><meta charset="utf-8"><title>Frame branch root</title>
<iframe src="/versioning/frame-branch-left"></iframe>
<iframe src="/versioning/frame-branch-right"></iframe>
<iframe src="{origin}/versioning/frame-branch-leaf?branch=cross"></iframe>"#
            ))
        }
        ("GET", "/versioning/frame-branch-left") => html(
            r#"<!doctype html><meta charset="utf-8"><title>Frame branch left</title>
<iframe src="/versioning/frame-branch-leaf?branch=left-a"></iframe>
<iframe src="/versioning/frame-branch-leaf?branch=left-b"></iframe>"#,
        ),
        ("GET", "/versioning/frame-branch-right") => html(
            r#"<!doctype html><meta charset="utf-8"><title>Frame branch right</title>
<iframe src="/versioning/frame-branch-leaf?branch=right-a"></iframe>"#,
        ),
        ("GET", "/versioning/frame-branch-leaf")
            if matches!(
                request.target.as_str(),
                "/versioning/frame-branch-leaf?branch=left-a"
                    | "/versioning/frame-branch-leaf?branch=left-b"
                    | "/versioning/frame-branch-leaf?branch=right-a"
                    | "/versioning/frame-branch-leaf?branch=cross"
            ) =>
        {
            html("<!doctype html><meta charset=\"utf-8\"><p>Finite branch leaf</p>")
        }
        ("GET", "/versioning/frame-depth") => {
            let level = request
                .target
                .strip_prefix("/versioning/frame-depth?level=")
                .and_then(|value| value.parse::<u8>().ok())
                .filter(|level| *level <= 9)?;
            let child = if level < 9 {
                format!(
                    "<iframe title=\"synthetic depth\" src=\"/versioning/frame-depth?level={}\"></iframe>",
                    level + 1
                )
            } else {
                String::new()
            };
            html(&format!(
                "<!doctype html><meta charset=\"utf-8\"><title>Depth {level}</title><p>synthetic depth {level}</p>{child}"
            ))
        }
        ("GET", "/versioning/activity") => html(include_str!(
            "../../../ferric-browser-engine-qt/tests/fixtures/versioning/activity.html"
        )),
        ("GET", "/versioning/activity/poll") => text(200, "OK", "synthetic-poll"),
        ("GET", "/versioning/activity/worker.js") => asset(
            "application/javascript; charset=utf-8",
            include_str!("../../../ferric-browser-engine-qt/tests/fixtures/versioning/worker.js"),
        ),
        ("GET", "/versioning/activity/sw.js") => asset(
            "application/javascript; charset=utf-8",
            include_str!("../../../ferric-browser-engine-qt/tests/fixtures/versioning/sw.js"),
        ),
        ("GET", "/versioning/archive") => {
            let origin = alternate_loopback_origin(request);
            html(
                &include_str!(
                    "../../../ferric-browser-engine-qt/tests/fixtures/versioning/archive.html"
                )
                .replace("{{REMOTE_BASE}}", &origin),
            )
        }
        ("GET", "/versioning/archive-meta") => html(&format!(
            "<!doctype html><meta http-equiv=\"refresh\" content=\"0;url={}/versioning/archive-redirect\"><title>Archive redirect</title>",
            alternate_loopback_origin(request)
        )),
        ("GET", "/versioning/archive.css") => {
            asset("text/css; charset=utf-8", "body { color: #123456; }")
        }
        ("GET", "/versioning/archive-image") => asset(
            "image/svg+xml",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>",
        ),
        ("GET", "/versioning/archive-frame") => {
            html("<!doctype html><title>Archive remote frame</title><p>synthetic frame</p>")
        }
        ("POST", "/versioning/archive-form") if !request.body.is_empty() => {
            text(200, "OK", "synthetic remote form received")
        }
        ("GET", "/versioning/archive-popup") => {
            html("<!doctype html><title>Archive popup</title><p>synthetic popup</p>")
        }
        ("GET", "/versioning/archive-redirect") => {
            super::redirect_response(302, "/versioning/document")
        }
        ("GET", "/versioning/archive-download") => {
            asset("application/octet-stream", "synthetic archive download")
        }
        ("GET", "/versioning/session/login") => html(include_str!(
            "../../../ferric-browser-engine-qt/tests/fixtures/versioning/login.html"
        )),
        ("POST", "/versioning/session/login") if request.body == b"account=synthetic" => {
            session.authenticated = true;
            let mut response = super::redirect_response(303, "/versioning/session/member");
            response.set_cookies = vec![
                "vb_session=synthetic; Path=/versioning/session; HttpOnly; SameSite=Lax",
                "vb_csrf=synthetic-csrf; Path=/versioning/session; SameSite=Lax",
            ];
            response
        }
        ("GET", "/versioning/session/member") if has_session(request, session) => html(
            include_str!("../../../ferric-browser-engine-qt/tests/fixtures/versioning/member.html"),
        ),
        ("POST", "/versioning/session/change")
            if has_session(request, session)
                && request
                    .headers
                    .get("x-vb-csrf")
                    .is_some_and(|value| value == "synthetic-csrf")
                && request.body == b"synthetic-change" =>
        {
            text(200, "OK", "synthetic change accepted")
        }
        ("POST", "/versioning/session/logout") if has_session(request, session) => {
            session.authenticated = false;
            let mut response = text(200, "OK", "synthetic session revoked");
            response.set_cookies = vec![
                "vb_session=; Path=/versioning/session; Max-Age=0; HttpOnly; SameSite=Lax",
                "vb_csrf=; Path=/versioning/session; Max-Age=0; SameSite=Lax",
            ];
            response
        }
        ("GET", "/versioning/cookies/secure") if request.secure_transport => {
            let mut response = html("<!doctype html><title>Secure cookie fixture</title>");
            response.set_cookies = vec![
                "vb_host=synthetic; Path=/versioning; Secure; HttpOnly; SameSite=Strict",
                "vb_path=synthetic; Path=/versioning/cookies; Secure; SameSite=Lax",
                "vb_partition=synthetic; Path=/versioning; Secure; SameSite=None; Partitioned",
            ];
            response
        }
        ("GET", "/versioning/cookies/secure") => {
            text(426, "Upgrade Required", "HTTPS fixture required")
        }
        ("GET", "/versioning/cookies/variants") if request.secure_transport => {
            let mut response = html("<!doctype html><title>Cookie variant fixture</title>");
            response.set_cookies = vec![
                "vb_same=host; Path=/versioning; Secure; SameSite=Strict",
                "vb_same=path; Path=/versioning/cookies; Secure; SameSite=Lax",
                "vb_expiry=synthetic; Path=/versioning; Max-Age=3600; Secure; SameSite=None",
                "vb_transient=synthetic; Path=/versioning; Secure; HttpOnly",
            ];
            response
        }
        ("GET", "/versioning/cookies/domain")
            if request.secure_transport
                && request
                    .headers
                    .get("host")
                    .is_some_and(|host| host.starts_with("localhost:")) =>
        {
            let mut response = html("<!doctype html><title>Domain cookie fixture</title>");
            response.set_cookies = vec![
                "vb_domain=synthetic; Domain=localhost; Path=/versioning; Secure; SameSite=Lax",
            ];
            response
        }
        ("GET", "/versioning/cookies/variants" | "/versioning/cookies/domain") => {
            text(426, "Upgrade Required", "HTTPS localhost fixture required")
        }
        ("GET", "/versioning/fault/slow") => text(200, "OK", "delayed synthetic response"),
        ("GET", "/versioning/fault/error") => text(503, "Service Unavailable", "synthetic failure"),
        (_, path) if path.starts_with("/versioning/") => {
            text(404, "Not Found", "versioning fixture route not found")
        }
        _ => return None,
    };
    Some(response)
}

fn has_session(request: &FixtureRequest, session: &SessionState) -> bool {
    session.authenticated
        && has_cookie(request, "vb_session", "synthetic")
        && has_cookie(request, "vb_csrf", "synthetic-csrf")
}

fn has_cookie(request: &FixtureRequest, name: &str, expected: &str) -> bool {
    request.headers.get("cookie").is_some_and(|header| {
        header.split(';').any(|part| {
            part.trim()
                .split_once('=')
                .is_some_and(|(key, value)| key == name && value == expected)
        })
    })
}

fn alternate_loopback_origin(request: &FixtureRequest) -> String {
    let port = request
        .headers
        .get("host")
        .and_then(|host| host.rsplit_once(':').map(|(_, port)| port))
        .and_then(|port| port.parse::<u16>().ok())
        .unwrap_or(if request.secure_transport { 443 } else { 80 });
    let scheme = if request.secure_transport {
        "https"
    } else {
        "http"
    };
    format!("{scheme}://localhost:{port}")
}

fn html(body: &str) -> FixtureResponse {
    asset("text/html; charset=utf-8", body)
}

fn text(status: u16, reason: &'static str, body: &str) -> FixtureResponse {
    let mut response = asset("text/plain; charset=utf-8", body);
    response.status = status;
    response.reason = reason;
    response
}

fn asset(content_type: &'static str, body: &str) -> FixtureResponse {
    FixtureResponse {
        status: 200,
        reason: "OK",
        content_type,
        body: body.as_bytes().to_vec(),
        location: None,
        websocket_accept: None,
        www_authenticate: false,
        websocket: false,
        set_cookies: Vec::new(),
        byte_range: None,
    }
}

fn media_silence_wav(request: &FixtureRequest) -> FixtureResponse {
    const SAMPLE_RATE: u32 = 8_000;
    const SAMPLE_COUNT: u32 = SAMPLE_RATE;
    let mut body = Vec::with_capacity(44 + SAMPLE_COUNT as usize);
    body.extend_from_slice(b"RIFF");
    body.extend_from_slice(&(36 + SAMPLE_COUNT).to_le_bytes());
    body.extend_from_slice(b"WAVEfmt ");
    body.extend_from_slice(&16_u32.to_le_bytes());
    body.extend_from_slice(&1_u16.to_le_bytes());
    body.extend_from_slice(&1_u16.to_le_bytes());
    body.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    body.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    body.extend_from_slice(&1_u16.to_le_bytes());
    body.extend_from_slice(&8_u16.to_le_bytes());
    body.extend_from_slice(b"data");
    body.extend_from_slice(&SAMPLE_COUNT.to_le_bytes());
    body.resize(44 + SAMPLE_COUNT as usize, 0x80);
    let total = body.len();
    let mut response = FixtureResponse {
        status: 200,
        reason: "OK",
        content_type: "audio/wav",
        body,
        location: None,
        websocket_accept: None,
        www_authenticate: false,
        websocket: false,
        set_cookies: Vec::new(),
        byte_range: Some(ByteRangeResponse::Full),
    };
    if let Some(range) = request
        .headers
        .get("range")
        .and_then(|value| parse_single_byte_range(value, total))
    {
        if let Ok((start, end)) = range {
            response.status = 206;
            response.reason = "Partial Content";
            response.body = response.body[start..=end].to_vec();
            response.byte_range = Some(ByteRangeResponse::Partial { start, end, total });
        } else {
            response.status = 416;
            response.reason = "Range Not Satisfiable";
            response.body.clear();
            response.byte_range = Some(ByteRangeResponse::Unsatisfiable { total });
        }
    }
    response
}

// This fixture deliberately supports one bounded range, enough for Chromium's
// byte-seeking request. Malformed or multipart ranges fall back to the full
// representation; a well-formed out-of-bounds range receives 416.
fn parse_single_byte_range(raw: &str, total: usize) -> Option<Result<(usize, usize), ()>> {
    let value = raw.strip_prefix("bytes=")?;
    if value.contains(',') {
        return None;
    }
    let (first, last) = value.split_once('-')?;
    if first.is_empty() {
        let suffix = last.parse::<usize>().ok()?;
        if suffix == 0 {
            return Some(Err(()));
        }
        return Some(Ok((total.saturating_sub(suffix), total - 1)));
    }
    let start = first.parse::<usize>().ok()?;
    let end = if last.is_empty() {
        total - 1
    } else {
        last.parse::<usize>().ok()?.min(total - 1)
    };
    if start >= total {
        Some(Err(()))
    } else if end < start {
        None
    } else {
        Some(Ok((start, end)))
    }
}
