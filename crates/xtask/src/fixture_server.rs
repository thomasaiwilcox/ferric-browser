use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_WEBSOCKET_FRAME_BYTES: usize = 8 * 1024;
const WEBSOCKET_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

#[derive(Debug)]
struct FixtureRequest {
    method: String,
    target: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug)]
struct FixtureResponse {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    body: Vec<u8>,
    location: Option<String>,
    websocket_accept: Option<String>,
    www_authenticate: bool,
    websocket: bool,
}

/// Runs the deterministic loopback fixture server.
///
/// Supported options are --bind ADDRESS (default 127.0.0.1:0), --once, and
/// --https. HTTPS uses an ephemeral test certificate and the local `socat` and
/// `openssl` commands; it is never used by the production browser.
pub fn run(arguments: &[String]) -> Result<(), String> {
    let mut bind = "127.0.0.1:0".to_owned();
    let mut once = false;
    let mut https = false;
    let mut exit_on_path = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--bind" => {
                index += 1;
                bind = arguments
                    .get(index)
                    .cloned()
                    .ok_or_else(|| "--bind requires ADDRESS".to_owned())?;
            }
            "--once" => once = true,
            "--https" => https = true,
            "--exit-on-path" => {
                index += 1;
                let path = arguments
                    .get(index)
                    .cloned()
                    .ok_or_else(|| "--exit-on-path requires PATH".to_owned())?;
                if !path.starts_with('/') || path.len() > 256 {
                    return Err("--exit-on-path requires a bounded absolute path".into());
                }
                exit_on_path = Some(path);
            }
            value => return Err(format!("unknown fixture-server option: {value}")),
        }
        index += 1;
    }

    validate_loopback_bind(&bind)?;
    if https {
        return run_https(&bind, once);
    }
    let listener =
        TcpListener::bind(&bind).map_err(|error| format!("could not bind {bind}: {error}"))?;
    run_http_listener(listener, once, "http", true, exit_on_path.as_deref())
}

fn validate_loopback_bind(bind: &str) -> Result<(), String> {
    let addresses = bind
        .to_socket_addrs()
        .map_err(|error| format!("fixture bind address is invalid: {error}"))?
        .collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|address| !address.ip().is_loopback()) {
        return Err("fixture server only permits loopback bind addresses".into());
    }
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn run_http_listener(
    listener: TcpListener,
    once: bool,
    scheme: &str,
    announce: bool,
    exit_on_path: Option<&str>,
) -> Result<(), String> {
    let address = listener
        .local_addr()
        .map_err(|error| format!("could not read fixture {scheme} address: {error}"))?;
    if announce {
        println!("RUSTBROWSER_FIXTURE_READY {scheme}://{address}");
        io::stdout()
            .flush()
            .map_err(|error| format!("could not flush fixture address: {error}"))?;
    }

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let request_path = match serve_connection(&mut stream) {
                    Ok(path) => Some(path),
                    Err(error) => {
                        eprintln!("fixture-server: {error}");
                        None
                    }
                };
                if exit_on_path.is_some() && request_path.is_some() {
                    println!(
                        "RUSTBROWSER_FIXTURE_EVENT path={}",
                        request_path.as_deref().unwrap_or_default()
                    );
                    io::stdout()
                        .flush()
                        .map_err(|error| format!("could not flush fixture event: {error}"))?;
                    if request_path.as_deref() == exit_on_path {
                        break;
                    }
                }
                if once {
                    break;
                }
            }
            Err(error) => {
                if once {
                    return Err(format!("fixture connection failed: {error}"));
                }
            }
        }
    }
    Ok(())
}

fn run_https(bind: &str, once: bool) -> Result<(), String> {
    if !command_available("openssl") || !command_available("socat") {
        return Err(
            "HTTPS fixtures require both openssl and socat; use the HTTP fixture or install the development tools"
                .into(),
        );
    }
    let backend = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("could not bind HTTPS fixture backend: {error}"))?;
    let backend_address = backend
        .local_addr()
        .map_err(|error| format!("could not read HTTPS backend address: {error}"))?;
    let probe = TcpListener::bind(bind)
        .map_err(|error| format!("could not reserve HTTPS fixture address {bind}: {error}"))?;
    let address = probe
        .local_addr()
        .map_err(|error| format!("could not read HTTPS fixture address: {error}"))?;
    drop(probe);
    if !address.ip().is_ipv4() {
        return Err("HTTPS fixture mode requires an IPv4 loopback bind address".into());
    }

    let directory = std::env::temp_dir().join(format!(
        "rustbrowser-fixture-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    ));
    fs::create_dir(&directory)
        .map_err(|error| format!("could not create HTTPS fixture directory: {error}"))?;
    let certificate = directory.join("fixture-cert.pem");
    let key = directory.join("fixture-key.pem");
    let certificate_status = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            key.to_str().unwrap_or_default(),
            "-out",
            certificate.to_str().unwrap_or_default(),
            "-days",
            "1",
            "-subj",
            "/CN=RustBrowser fixture",
            "-addext",
            "subjectAltName=DNS:localhost,IP:127.0.0.1",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("could not generate HTTPS fixture certificate: {error}"))?;
    if !certificate_status.success() {
        let _ = fs::remove_dir_all(&directory);
        return Err("openssl could not generate the HTTPS fixture certificate".into());
    }
    let listener = format!(
        "OPENSSL-LISTEN:{},bind=127.0.0.1,cert={},key={},verify=0,reuseaddr{}",
        address.port(),
        certificate.display(),
        key.display(),
        if once { "" } else { ",fork" }
    );
    let backend_thread = thread::spawn(move || {
        if let Err(error) = run_http_listener(backend, once, "https-backend", false, None) {
            eprintln!("fixture-server: HTTPS backend: {error}");
        }
    });
    let mut proxy = match Command::new("socat")
        .arg(listener)
        .arg(format!("TCP:{backend_address}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(proxy) => proxy,
        Err(error) => {
            let _ = fs::remove_dir_all(&directory);
            return Err(format!("could not start HTTPS fixture proxy: {error}"));
        }
    };
    println!("RUSTBROWSER_FIXTURE_READY https://{address}");
    io::stdout()
        .flush()
        .map_err(|error| format!("could not flush fixture address: {error}"))?;
    let status = proxy
        .wait()
        .map_err(|error| format!("HTTPS fixture proxy failed: {error}"))?;
    if once {
        let _ = backend_thread.join();
    }
    let _ = fs::remove_dir_all(&directory);
    if status.success() {
        Ok(())
    } else {
        Err("HTTPS fixture proxy exited unsuccessfully".into())
    }
}

fn command_available(name: &str) -> bool {
    Command::new(name)
        .arg(if name == "openssl" { "version" } else { "-V" })
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn serve_connection(stream: &mut TcpStream) -> Result<String, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("could not configure read timeout: {error}"))?;
    let request = read_request(stream)?;
    let response = route_request(&request);
    write_response(stream, &response)?;
    if response.websocket {
        serve_websocket(stream)?;
    }
    let path = request
        .target
        .split_once('?')
        .map_or_else(|| request.target.clone(), |(path, _)| path.to_owned());
    Ok(path)
}

fn read_request(stream: &mut TcpStream) -> Result<FixtureRequest, String> {
    let mut bytes = Vec::new();
    let header_end = loop {
        let mut chunk = [0_u8; 4096];
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("could not read request: {error}"))?;
        if read == 0 {
            return Err("fixture client closed before request headers".into());
        }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.len() > MAX_HEADER_BYTES + MAX_BODY_BYTES {
            return Err("fixture request exceeds the size limit".into());
        }
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
        if bytes.len() > MAX_HEADER_BYTES {
            return Err("fixture request headers exceed the size limit".into());
        }
    };

    let header_text = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| "fixture request headers are not UTF-8".to_owned())?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "fixture request line is missing".to_owned())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| "fixture request method is missing".to_owned())?
        .to_owned();
    let target = request_parts
        .next()
        .ok_or_else(|| "fixture request target is missing".to_owned())?
        .to_owned();
    let version = request_parts
        .next()
        .ok_or_else(|| "fixture request version is missing".to_owned())?;
    if request_parts.next().is_some() || version != "HTTP/1.1" {
        return Err("fixture request line is invalid".into());
    }
    if method.len() > 16 || target.len() > 8 * 1024 {
        return Err("fixture request method or target is too long".into());
    }

    let mut headers = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| "fixture header is invalid".to_owned())?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if name.is_empty() || name.len() > 128 || value.len() > 8 * 1024 {
            return Err("fixture header is too long".into());
        }
        headers.insert(name, value.to_owned());
    }
    let content_length = headers.get("content-length").map_or(Ok(0_usize), |value| {
        value
            .parse::<usize>()
            .map_err(|_| "fixture content length is invalid".to_owned())
    })?;
    if content_length > MAX_BODY_BYTES {
        return Err("fixture request body exceeds the size limit".into());
    }

    while bytes.len() < header_end + content_length {
        let mut chunk = [0_u8; 4096];
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("could not read fixture body: {error}"))?;
        if read == 0 {
            return Err("fixture client closed before request body".into());
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    Ok(FixtureRequest {
        method,
        target,
        headers,
        body: bytes[header_end..header_end + content_length].to_vec(),
    })
}

fn write_response(stream: &mut TcpStream, response: &FixtureResponse) -> Result<(), String> {
    let location = response
        .location
        .as_ref()
        .map_or(String::new(), |value| format!("Location: {value}\r\n"));
    let auth = if response.www_authenticate {
        "WWW-Authenticate: Basic realm=\"RustBrowser fixture\"\r\n"
    } else {
        ""
    };
    let websocket = if response.websocket {
        "Connection: Upgrade\r\nUpgrade: websocket\r\n"
    } else {
        "Connection: close\r\n"
    };
    let websocket_accept = response
        .websocket_accept
        .as_ref()
        .map_or(String::new(), |value| {
            format!("Sec-WebSocket-Accept: {value}\r\n")
        });
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\n{}{}{}{}Access-Control-Allow-Origin: *\r\n\r\n",
        response.status,
        response.reason,
        response.content_type,
        response.body.len(),
        websocket,
        location,
        auth,
        websocket_accept
    );
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(&response.body))
        .map_err(|error| format!("could not write fixture response: {error}"))
}

#[allow(clippy::too_many_lines)]
fn route_request(request: &FixtureRequest) -> FixtureResponse {
    let path = request
        .target
        .split_once('?')
        .map_or(request.target.as_str(), |(path, _)| path);
    if path == "/websocket" {
        return websocket_response(request);
    }
    match (request.method.as_str(), path) {
        ("GET", "/") => html_response(
            "RustBrowser local fixture",
            r#"<h1>RustBrowser fixture</h1>
               <a href="/redirect">redirect</a>
               <a href="/post">post form</a>
               <a href="/post-307">post 307</a>
               <a href="/download">download</a>
               <a href="/auth">auth</a>
               <a href="/popup">popup</a>
               <a href="/permissions">permissions</a>
               <a href="/file-form">file form</a>
               <a href="/editable">editable</a>
               <a href="/frames">frames</a>
               <a href="/shadow">shadow</a>
               <a href="/service-worker">service worker</a>
               <a href="/heavy">heavy</a>
               <a href="/cross-origin">cross origin</a>"#,
        ),
        ("GET", "/blocking") => html_response(
            "RustBrowser blocking fixture",
            r#"<img alt="blocked resource" src="http://127.0.0.1:18774/__rustbrowser_blocked__">
               <script src="http://127.0.0.1:18774/__rustbrowser_excepted__"></script>"#,
        ),
        ("GET", "/redirect") => redirect_response(302, "/final?from=redirect"),
        ("GET", "/final") => html_response(
            "Redirect final",
            "<h1>Redirect final</h1><p id=\"result\">redirected</p>",
        ),
        ("GET", "/post") => html_response(
            "POST form",
            r#"<form method="post" action="/post-result">
                 <input name="fixture" value="submitted">
                 <button type="submit">submit</button>
               </form>"#,
        ),
        ("GET", "/post-307") => html_response(
            "POST 307 form",
            r#"<form method="post" action="/post-307-result">
                 <input name="fixture" value="submitted">
                 <button type="submit">submit 307</button>
               </form>"#,
        ),
        ("POST", "/post-result") if !request.body.is_empty() => {
            redirect_response(303, "/post-result?accepted=1")
        }
        ("POST", "/post-307-result") if !request.body.is_empty() => {
            redirect_response(307, "/post-307-result?replayed=1")
        }
        ("GET", "/post-result") => html_response(
            "POST result",
            "<h1>POST result</h1><p id=\"result\">accepted without replay</p>",
        ),
        ("GET", "/post-307-result") => html_response(
            "POST 307 result",
            "<h1>POST 307 result</h1><p id=\"result\">engine preserved method</p>",
        ),
        ("GET", "/download") => FixtureResponse {
            status: 200,
            reason: "OK",
            content_type: "application/octet-stream",
            body: b"RustBrowser fixture download\n".to_vec(),
            location: None,
            websocket_accept: None,
            www_authenticate: false,
            websocket: false,
        },
        ("GET", "/auth") => FixtureResponse {
            status: 401,
            reason: "Unauthorized",
            content_type: "text/html; charset=utf-8",
            body: b"<h1>fixture auth</h1>".to_vec(),
            location: None,
            websocket_accept: None,
            www_authenticate: true,
            websocket: false,
        },
        ("GET", "/popup") => html_response(
            "Popup fixture",
            r#"<button onclick="window.open('/popup-target', 'fixture')">open popup</button>"#,
        ),
        ("GET", "/popup-target") => html_response(
            "Popup target",
            "<h1>Popup target</h1><p id=\"opener\">popup fixture</p>",
        ),
        ("GET", "/permissions") => html_response(
            "Permission fixture",
            r#"<button onclick="navigator.geolocation.getCurrentPosition(
                 () => {}, () => {})">request location</button>"#,
        ),
        ("GET", "/file-form") => html_response(
            "File form",
            r#"<form enctype="multipart/form-data" method="post" action="/file-result">
                 <input type="file" name="fixture-file"><button type="submit">upload</button>
               </form>"#,
        ),
        ("POST", "/file-result") if !request.body.is_empty() => html_response(
            "File result",
            "<h1>File result</h1><p id=\"result\">received</p>",
        ),
        ("GET", "/editable") => html_response(
            "Editable fixture",
            r#"<textarea id="editor" autofocus oninput="if (this.value === 'native-input') window.location = '/input-result?value=native-input'">fixture text</textarea>
               <div id="contenteditable" contenteditable="true">editable fixture</div>"#,
        ),
        ("GET", "/input-result") if request.target.contains("value=native-input") => html_response(
            "Input result",
            "<h1 id=\"result\">native input accepted</h1>",
        ),
        ("GET", "/frames") => html_response(
            "Frames fixture",
            r#"<iframe title="fixture frame" src="/frame-child"></iframe>
               <iframe title="nested fixture" src="/frame-child?nested=1"></iframe>"#,
        ),
        ("GET", "/frame-child") => html_response(
            "Frame child",
            "<h1 id=\"frame-heading\">Frame fixture</h1><a href=\"/final\">frame link</a>",
        ),
        ("GET", "/shadow") => html_response(
            "Shadow fixture",
            r#"<div id="shadow-host"></div><script>
               const host = document.getElementById('shadow-host');
               const root = host.attachShadow({mode:'open'});
               root.innerHTML = '<a href="/final">shadow link</a>';
               </script>"#,
        ),
        ("GET", "/service-worker") => html_response(
            "Service worker fixture",
            r#"<script>
               navigator.serviceWorker.register('/service-worker.js');
               </script><h1 id="sw-page">Service worker fixture</h1>"#,
        ),
        ("GET", "/service-worker.js") => script_response(
            "self.addEventListener('fetch', event => {\n  event.respondWith(fetch(event.request));\n});\n",
        ),
        ("GET", "/heavy") => heavy_response(),
        ("GET", "/cross-origin") => cross_origin_response(request),
        ("GET", "/cross-origin-target") => html_response(
            "Cross-origin target",
            "<h1 id=\"cross-origin\">Cross-origin target</h1>",
        ),
        _ => FixtureResponse {
            status: 404,
            reason: "Not Found",
            content_type: "text/plain; charset=utf-8",
            body: b"fixture route not found\n".to_vec(),
            location: None,
            websocket_accept: None,
            www_authenticate: false,
            websocket: false,
        },
    }
}

fn html_response(title: &'static str, body: &str) -> FixtureResponse {
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title></head>\
         <body>{body}</body></html>"
    );
    FixtureResponse {
        status: 200,
        reason: "OK",
        content_type: "text/html; charset=utf-8",
        body: body.into_bytes(),
        location: None,
        websocket_accept: None,
        www_authenticate: false,
        websocket: false,
    }
}

fn redirect_response(status: u16, location: &'static str) -> FixtureResponse {
    FixtureResponse {
        status,
        reason: if status == 303 {
            "See Other"
        } else if status == 307 {
            "Temporary Redirect"
        } else {
            "Found"
        },
        content_type: "text/plain; charset=utf-8",
        body: format!("redirecting to {location}\n").into_bytes(),
        location: Some(location.to_owned()),
        websocket_accept: None,
        www_authenticate: false,
        websocket: false,
    }
}

fn script_response(body: &str) -> FixtureResponse {
    FixtureResponse {
        status: 200,
        reason: "OK",
        content_type: "application/javascript; charset=utf-8",
        body: body.as_bytes().to_vec(),
        location: None,
        websocket_accept: None,
        www_authenticate: false,
        websocket: false,
    }
}

fn heavy_response() -> FixtureResponse {
    let mut body = String::from("<h1>Heavy fixture</h1><main>");
    for index in 0..512 {
        let _ = write!(
            body,
            "<p data-fixture=\"{index}\">bounded fixture row {index}</p>"
        );
    }
    body.push_str("</main>");
    html_response("Heavy fixture", &body)
}

fn cross_origin_response(request: &FixtureRequest) -> FixtureResponse {
    let port = request
        .headers
        .get("host")
        .and_then(|host| host.rsplit_once(':').map(|(_, port)| port))
        .filter(|port| {
            !port.is_empty() && port.len() <= 5 && port.chars().all(|c| c.is_ascii_digit())
        })
        .unwrap_or("443");
    let body = format!(
        "<h1>Cross-origin fixture</h1><a id=\"cross-origin-link\" href=\"https://localhost:{port}/cross-origin-target\">localhost target</a>"
    );
    html_response("Cross-origin fixture", &body)
}

fn websocket_response(request: &FixtureRequest) -> FixtureResponse {
    let upgrade = request
        .headers
        .get("upgrade")
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"));
    let connection = request.headers.get("connection").is_some_and(|value| {
        value
            .split(',')
            .any(|part| part.trim().eq_ignore_ascii_case("upgrade"))
    });
    let Some(key) = request.headers.get("sec-websocket-key") else {
        return websocket_rejection();
    };
    if request.method != "GET" || !upgrade || !connection || key.len() > 128 {
        return websocket_rejection();
    }
    let mut input = Vec::with_capacity(key.len() + WEBSOCKET_GUID.len());
    input.extend_from_slice(key.as_bytes());
    input.extend_from_slice(WEBSOCKET_GUID);
    FixtureResponse {
        status: 101,
        reason: "Switching Protocols",
        content_type: "application/octet-stream",
        body: Vec::new(),
        location: None,
        websocket_accept: Some(base64_encode(&sha1(&input))),
        www_authenticate: false,
        websocket: true,
    }
}

fn websocket_rejection() -> FixtureResponse {
    FixtureResponse {
        status: 426,
        reason: "Upgrade Required",
        content_type: "text/plain; charset=utf-8",
        body: b"fixture websocket upgrade required\n".to_vec(),
        location: None,
        websocket_accept: None,
        www_authenticate: false,
        websocket: false,
    }
}

fn serve_websocket(stream: &mut TcpStream) -> Result<(), String> {
    let mut header = [0_u8; 2];
    if stream.read_exact(&mut header).is_err() {
        return Ok(());
    }
    let masked = header[1] & 0x80 != 0;
    let mut length = usize::from(header[1] & 0x7f);
    if length == 126 {
        let mut extended = [0_u8; 2];
        stream
            .read_exact(&mut extended)
            .map_err(|error| error.to_string())?;
        length = usize::from(u16::from_be_bytes(extended));
    } else if length == 127 {
        let mut extended = [0_u8; 8];
        stream
            .read_exact(&mut extended)
            .map_err(|error| error.to_string())?;
        let value = u64::from_be_bytes(extended);
        length = usize::try_from(value).unwrap_or(usize::MAX);
    }
    if length > MAX_WEBSOCKET_FRAME_BYTES || !masked {
        return Ok(());
    }
    let mut mask = [0_u8; 4];
    stream
        .read_exact(&mut mask)
        .map_err(|error| error.to_string())?;
    let mut payload = vec![0_u8; length];
    stream
        .read_exact(&mut payload)
        .map_err(|error| error.to_string())?;
    for (index, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[index % 4];
    }
    let response = b"fixture:websocket";
    let response_length = u8::try_from(response.len()).expect("bounded websocket response");
    let frame = [vec![0x81_u8, response_length], response.to_vec()].concat();
    stream.write_all(&frame).map_err(|error| error.to_string())
}

#[allow(clippy::many_single_char_names)]
fn sha1(input: &[u8]) -> [u8; 20] {
    let mut message = input.to_vec();
    let bit_length = (message.len() as u64).saturating_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());
    let mut state = [
        0x6745_2301_u32,
        0xefcd_ab89,
        0x98ba_dcfe,
        0x1032_5476,
        0xc3d2_e1f0,
    ];
    for chunk in message.chunks_exact(64) {
        let mut words = [0_u32; 80];
        for (index, word) in words[..16].iter_mut().enumerate() {
            let start = index * 4;
            *word = u32::from_be_bytes(chunk[start..start + 4].try_into().expect("word"));
        }
        for index in 16..80 {
            words[index] =
                (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                    .rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) =
            (state[0], state[1], state[2], state[3], state[4]);
        for (index, word) in words.iter().enumerate() {
            let (function, constant) = match index {
                0..=19 => ((b & c) | ((!b) & d), 0x5a82_7999),
                20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
                _ => (b ^ c ^ d, 0xca62_c1d6),
            };
            let next = a
                .rotate_left(5)
                .wrapping_add(function)
                .wrapping_add(e)
                .wrapping_add(constant)
                .wrapping_add(*word);
            (a, b, c, d, e) = (next, a, b.rotate_left(30), c, d);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
    }
    let mut result = [0_u8; 20];
    for (index, word) in state.iter().enumerate() {
        result[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    result
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        result.push(TABLE[usize::from(first >> 2)] as char);
        result.push(TABLE[usize::from((first & 0x03) << 4 | second >> 4)] as char);
        result.push(if chunk.len() > 1 {
            TABLE[usize::from((second & 0x0f) << 2 | third >> 6)] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            TABLE[usize::from(third & 0x3f)] as char
        } else {
            '='
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: &str, target: &str, body: &[u8]) -> FixtureRequest {
        FixtureRequest {
            method: method.to_owned(),
            target: target.to_owned(),
            headers: BTreeMap::new(),
            body: body.to_vec(),
        }
    }

    #[test]
    fn redirect_route_preserves_explicit_target() {
        let response = route_request(&request("GET", "/redirect", &[]));
        assert_eq!(response.status, 302);
        assert_eq!(response.location.as_deref(), Some("/final?from=redirect"));
    }

    #[test]
    fn post_routes_distinguish_redirect_semantics() {
        let body = b"fixture=submitted";
        let post = route_request(&request("POST", "/post-result", body));
        assert_eq!(post.status, 303);
        assert_eq!(post.location.as_deref(), Some("/post-result?accepted=1"));
        let replay = route_request(&request("POST", "/post-307-result", body));
        assert_eq!(replay.status, 307);
        assert_eq!(
            replay.location.as_deref(),
            Some("/post-307-result?replayed=1")
        );
    }

    #[test]
    fn unsupported_or_empty_post_routes_do_not_claim_success() {
        assert_eq!(
            route_request(&request("GET", "/post-result", &[])).status,
            200
        );
        assert_eq!(
            route_request(&request("POST", "/post-result", &[])).status,
            404
        );
        assert_eq!(route_request(&request("GET", "/missing", &[])).status, 404);
    }

    #[test]
    fn sensitive_fixture_routes_are_obviously_local_and_bounded() {
        let auth = route_request(&request("GET", "/auth", &[]));
        assert!(auth.www_authenticate);
        assert!(auth.body.len() < MAX_BODY_BYTES);
        let root = route_request(&request("GET", "/", &[]));
        let root = String::from_utf8(root.body).expect("fixture HTML");
        assert!(root.contains("/redirect"));
        assert!(root.contains("/post"));
    }

    #[test]
    fn browser_behavior_routes_are_bounded_and_deterministic() {
        for path in [
            "/blocking",
            "/file-form",
            "/editable",
            "/frames",
            "/frame-child",
            "/shadow",
            "/service-worker",
            "/service-worker.js",
            "/heavy",
            "/cross-origin-target",
        ] {
            let response = route_request(&request("GET", path, &[]));
            assert_eq!(response.status, 200, "{path}");
            assert!(response.body.len() <= MAX_BODY_BYTES, "{path}");
        }
        let input_result = route_request(&request("GET", "/input-result?value=native-input", &[]));
        assert_eq!(input_result.status, 200);
        assert!(String::from_utf8_lossy(&input_result.body).contains("native input accepted"));
        let heavy = route_request(&request("GET", "/heavy", &[]));
        assert!(
            heavy
                .body
                .windows(b"data-fixture=\"511\"".len())
                .any(|window| { window == b"data-fixture=\"511\"" })
        );
        let blocking = String::from_utf8(route_request(&request("GET", "/blocking", &[])).body)
            .expect("blocking fixture HTML");
        assert!(blocking.contains("127.0.0.1:18774/__rustbrowser_blocked__"));
        assert!(blocking.contains("127.0.0.1:18774/__rustbrowser_excepted__"));
        assert!(blocking.contains("__rustbrowser_excepted__"));
    }

    #[test]
    fn cross_origin_fixture_uses_the_certificate_alias_as_a_distinct_origin() {
        let mut request = request("GET", "/cross-origin", &[]);
        request
            .headers
            .insert("host".into(), "127.0.0.1:8443".into());
        let response = route_request(&request);
        let body = String::from_utf8(response.body).expect("fixture HTML");
        assert!(body.contains("https://localhost:8443/cross-origin-target"));
        assert!(body.contains("cross-origin-link"));
    }

    #[test]
    fn websocket_handshake_uses_the_rfc_acceptance_value() {
        let mut request = request("GET", "/websocket", &[]);
        request.headers.insert("upgrade".into(), "websocket".into());
        request
            .headers
            .insert("connection".into(), "Upgrade".into());
        request.headers.insert(
            "sec-websocket-key".into(),
            "dGhlIHNhbXBsZSBub25jZQ==".into(),
        );
        let response = route_request(&request);
        assert_eq!(response.status, 101);
        assert!(response.websocket);
        assert_eq!(
            response.websocket_accept.as_deref(),
            Some("s3pPLMBiTxaQ9kYGzzhZRbK+xOo=")
        );
    }

    #[test]
    fn fixture_binding_rejects_non_loopback_addresses() {
        assert!(validate_loopback_bind("127.0.0.1:0").is_ok());
        assert!(validate_loopback_bind("[::1]:0").is_ok());
        assert!(validate_loopback_bind("0.0.0.0:0").is_err());
    }
}
