use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

mod versioning_fixtures;

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
    secure_transport: bool,
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
    set_cookies: Vec<&'static str>,
    byte_range: Option<ByteRangeResponse>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ByteRangeResponse {
    Full,
    Partial {
        start: usize,
        end: usize,
        total: usize,
    },
    Unsatisfiable {
        total: usize,
    },
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
        return run_https(&bind, once, exit_on_path.as_deref());
    }
    let listener =
        TcpListener::bind(&bind).map_err(|error| format!("could not bind {bind}: {error}"))?;
    run_http_listener(listener, once, "http", false, true, exit_on_path.as_deref())
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
    secure_transport: bool,
    announce: bool,
    exit_on_path: Option<&str>,
) -> Result<(), String> {
    let address = listener
        .local_addr()
        .map_err(|error| format!("could not read fixture {scheme} address: {error}"))?;
    if announce {
        println!("FERRIC_BROWSER_FIXTURE_READY {scheme}://{address}");
        io::stdout()
            .flush()
            .map_err(|error| format!("could not flush fixture address: {error}"))?;
    }

    let mut versioning_state = versioning_fixtures::SessionState::default();
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let request_path =
                    match serve_connection(&mut stream, secure_transport, &mut versioning_state) {
                        Ok(path) => Some(path),
                        Err(error) => {
                            eprintln!("fixture-server: {error}");
                            None
                        }
                    };
                if exit_on_path.is_some() && request_path.is_some() {
                    println!(
                        "FERRIC_BROWSER_FIXTURE_EVENT path={}",
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

// Certificate setup, proxy supervision and teardown form one ownership scope.
#[allow(clippy::too_many_lines)]
fn run_https(bind: &str, once: bool, exit_on_path: Option<&str>) -> Result<(), String> {
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

    let directory = super::DisposableDirectory::new("ferric-browser-fixture")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("could not protect HTTPS fixture directory: {error}"))?;
    }
    let certificate = directory.path().join("fixture-cert.pem");
    let key = directory.path().join("fixture-key.pem");
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
            "/CN=Ferric Browser fixture",
            "-addext",
            "subjectAltName=DNS:localhost,IP:127.0.0.1",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("could not generate HTTPS fixture certificate: {error}"))?;
    if !certificate_status.success() {
        return Err("openssl could not generate the HTTPS fixture certificate".into());
    }
    let listener = format!(
        "OPENSSL-LISTEN:{},bind=127.0.0.1,cert={},key={},verify=0,reuseaddr{}",
        address.port(),
        certificate.display(),
        key.display(),
        if once { "" } else { ",fork" }
    );
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
            return Err(format!("could not start HTTPS fixture proxy: {error}"));
        }
    };
    let shutdown_on_path = exit_on_path.is_some() && !once;
    let exit_on_path = exit_on_path.map(str::to_owned);
    let backend_thread = thread::spawn(move || {
        run_http_listener(
            backend,
            once,
            "https-backend",
            true,
            false,
            exit_on_path.as_deref(),
        )
    });
    println!("FERRIC_BROWSER_FIXTURE_READY https://{address}");
    println!("FERRIC_BROWSER_FIXTURE_CERT {}", certificate.display());
    io::stdout()
        .flush()
        .map_err(|error| format!("could not flush fixture address: {error}"))?;
    if shutdown_on_path {
        while !backend_thread.is_finished() {
            if let Some(status) = proxy
                .try_wait()
                .map_err(|error| format!("could not poll HTTPS fixture proxy: {error}"))?
            {
                return Err(format!(
                    "HTTPS fixture proxy exited before shutdown: {status}"
                ));
            }
            thread::sleep(Duration::from_millis(20));
        }
        let backend_result = backend_thread
            .join()
            .map_err(|_| "HTTPS fixture backend panicked".to_owned())?;
        let _ = proxy.kill();
        let _ = proxy.wait();
        return backend_result;
    }
    let status = proxy
        .wait()
        .map_err(|error| format!("HTTPS fixture proxy failed: {error}"))?;
    if once {
        // A TLS handshake can fail before socat opens the HTTP backend. The
        // one-shot backend still needs a bounded request to leave its accept
        // loop so the temporary certificate directory is removed.
        if !backend_thread.is_finished() {
            if let Ok(mut stream) =
                TcpStream::connect_timeout(&backend_address, Duration::from_millis(250))
            {
                let _ = stream.write_all(
                    b"GET /versioning/fault/close HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                );
            }
        }
        backend_thread
            .join()
            .map_err(|_| "HTTPS fixture backend panicked".to_owned())??;
    }
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

fn serve_connection(
    stream: &mut TcpStream,
    secure_transport: bool,
    versioning_state: &mut versioning_fixtures::SessionState,
) -> Result<String, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("could not configure read timeout: {error}"))?;
    let request = read_request(stream, secure_transport)?;
    let path = request
        .target
        .split_once('?')
        .map_or_else(|| request.target.clone(), |(path, _)| path.to_owned());
    if path == "/versioning/form/result" {
        eprintln!(
            "fixture-server: form result method={} body_bytes={}",
            request.method,
            request.body.len()
        );
    }
    if matches!(
        path.as_str(),
        "/versioning/redirect-303/submit"
            | "/versioning/redirect-303/result"
            | "/versioning/redirect-307/submit"
            | "/versioning/redirect-307/result"
    ) {
        eprintln!(
            "fixture-server: redirect path={} method={} body_bytes={}",
            path,
            request.method,
            request.body.len()
        );
    }
    if path == "/versioning/fault/close" {
        return Ok(path);
    }
    if request.method == "GET" && path == "/versioning/slow-download" {
        write_slow_download(stream)?;
        return Ok(path);
    }
    let response = route_request_with_state(&request, versioning_state);
    if path == "/versioning/fault/slow" {
        thread::sleep(Duration::from_millis(750));
    }
    write_response(stream, &response)?;
    if response.websocket {
        serve_websocket(stream)?;
    }
    Ok(path)
}

// A bounded transfer gives native shutdown tests a real in-progress request.
// It is deliberately local, synthetic, and slow enough to exercise the UI
// before the terminal Qt callback can run.
fn write_slow_download(stream: &mut TcpStream) -> Result<(), String> {
    const CHUNK_BYTES: usize = 32 * 1024;
    const CHUNKS: usize = 256;
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\n\
         Content-Disposition: attachment; filename=synthetic-slow.bin\r\n\
         Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        CHUNK_BYTES * CHUNKS
    );
    stream
        .write_all(header.as_bytes())
        .map_err(|error| format!("could not write slow-download headers: {error}"))?;
    let chunk = vec![b'F'; CHUNK_BYTES];
    for _ in 0..CHUNKS {
        stream
            .write_all(&chunk)
            .map_err(|error| format!("slow-download client closed: {error}"))?;
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn read_request(stream: &mut TcpStream, secure_transport: bool) -> Result<FixtureRequest, String> {
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
        secure_transport,
    })
}

fn write_response(stream: &mut TcpStream, response: &FixtureResponse) -> Result<(), String> {
    let location = response
        .location
        .as_ref()
        .map_or(String::new(), |value| format!("Location: {value}\r\n"));
    let auth = if response.www_authenticate {
        "WWW-Authenticate: Basic realm=\"Ferric Browser fixture\"\r\n"
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
    let mut cookie_headers = String::new();
    for cookie in &response.set_cookies {
        write!(&mut cookie_headers, "Set-Cookie: {cookie}\r\n")
            .map_err(|error| format!("could not format fixture cookie header: {error}"))?;
    }
    let byte_range_headers = match response.byte_range {
        None => String::new(),
        Some(ByteRangeResponse::Full) => "Accept-Ranges: bytes\r\n".to_owned(),
        Some(ByteRangeResponse::Partial { start, end, total }) => {
            format!("Accept-Ranges: bytes\r\nContent-Range: bytes {start}-{end}/{total}\r\n")
        }
        Some(ByteRangeResponse::Unsatisfiable { total }) => {
            format!("Accept-Ranges: bytes\r\nContent-Range: bytes */{total}\r\n")
        }
    };
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\n{}{}{}{}{}{}Access-Control-Allow-Origin: *\r\n\r\n",
        response.status,
        response.reason,
        response.content_type,
        response.body.len(),
        websocket,
        location,
        auth,
        websocket_accept,
        cookie_headers,
        byte_range_headers
    );
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(&response.body))
        .map_err(|error| format!("could not write fixture response: {error}"))
}

#[allow(clippy::too_many_lines)]
fn route_request_with_state(
    request: &FixtureRequest,
    versioning_state: &mut versioning_fixtures::SessionState,
) -> FixtureResponse {
    let path = request
        .target
        .split_once('?')
        .map_or(request.target.as_str(), |(path, _)| path);
    if path == "/websocket" {
        return websocket_response(request);
    }
    if let Some(response) = versioning_fixtures::route(request, path, versioning_state) {
        return response;
    }
    match (request.method.as_str(), path) {
        ("GET", "/") => html_response(
            "Ferric Browser local fixture",
            r#"<h1>Ferric Browser fixture</h1>
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
        ("GET", "/spatial-grid") => html_response(
            "Spatial grid fixture",
            r#"<style>
                 body { margin: 0; min-height: 100vh; font: 20px sans-serif; }
                 #surface { position: relative; width: 100vw; height: 100vh; }
                 button, canvas { position: absolute; }
                 #left { left: 8%; top: 12%; }
                 #right { right: 8%; top: 12%; }
                 #middle { left: 46%; top: 48%; }
                 canvas { left: 25%; top: 30%; width: 50%; height: 45%; border: 2px solid #333; }
                 #event-report { position: fixed; left: 8px; bottom: 8px; max-width: 90vw;
                   padding: 4px; background: #fff8; }
               </style>
               <div id="surface">
                 <button id="left">left</button><button id="right">right</button>
                 <button id="middle">middle</button><canvas id="grid-canvas" width="640" height="360"></canvas>
                 <iframe title="distinct-origin" src="http://127.0.0.1:18774/spatial-grid-frame"
                   style="position:absolute;right:4%;bottom:4%;width:30%;height:20%"></iframe>
                 <output id="event-report">events: none</output>
               </div>
               <script>
                 const report = document.getElementById('event-report');
                 const events = [];
                 function record(e) {
                   events.push(e.type + ':' + e.button + '@' + Math.round(e.clientX) + ',' + Math.round(e.clientY));
                   while (events.length > 32) events.shift();
                   report.textContent = 'events: ' + events.join(' | ');
                 }
                 for (const node of document.querySelectorAll('button,canvas')) {
                   for (const type of ['mousemove','mousedown','mouseup','click','contextmenu'])
                     node.addEventListener(type, record);
                 }
               </script>"#,
        ),
        ("GET", "/spatial-grid-frame") => html_response(
            "Spatial grid cross-origin frame",
            r#"<button id="frame-target" style="margin:30px;width:180px;height:80px">frame target</button>
               <output id="frame-report">events: none</output>
               <script>
                 const target = document.getElementById('frame-target');
                 const report = document.getElementById('frame-report');
                 const events = [];
                 for (const type of ['mousemove','mousedown','mouseup','click','contextmenu'])
                   target.addEventListener(type, e => {
                     events.push(type + ':' + e.button);
                     while (events.length > 16) events.shift();
                     report.textContent = 'events: ' + events.join(' | ');
                   });
               </script>"#,
        ),
        ("GET", "/blocking") => html_response(
            "Ferric Browser blocking fixture",
            r#"<img alt="blocked resource" src="http://127.0.0.1:18774/__ferric_browser_blocked__">
               <script src="http://127.0.0.1:18774/__ferric_browser_excepted__"></script>"#,
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
            body: b"Ferric Browser fixture download\n".to_vec(),
            location: None,
            websocket_accept: None,
            www_authenticate: false,
            websocket: false,
            set_cookies: Vec::new(),
            byte_range: None,
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
            set_cookies: Vec::new(),
            byte_range: None,
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
            set_cookies: Vec::new(),
            byte_range: None,
        },
    }
}

#[cfg(test)]
fn route_request(request: &FixtureRequest) -> FixtureResponse {
    route_request_with_state(request, &mut versioning_fixtures::SessionState::default())
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
        set_cookies: Vec::new(),
        byte_range: None,
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
        set_cookies: Vec::new(),
        byte_range: None,
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
        set_cookies: Vec::new(),
        byte_range: None,
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
        set_cookies: Vec::new(),
        byte_range: None,
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
        set_cookies: Vec::new(),
        byte_range: None,
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
            secure_transport: false,
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
    fn versioning_post_redirects_require_the_expected_method_and_body() {
        let body = b"redirect=synthetic";
        let see_other = route_request(&request("POST", "/versioning/redirect-303/submit", body));
        assert_eq!(see_other.status, 303);
        assert_eq!(
            see_other.location.as_deref(),
            Some("/versioning/redirect-303/result")
        );
        assert_eq!(
            route_request(&request("GET", "/versioning/redirect-303/result", &[])).status,
            200
        );
        assert_eq!(
            route_request(&request("POST", "/versioning/redirect-303/result", body)).status,
            404
        );
        let preserve = route_request(&request("POST", "/versioning/redirect-307/submit", body));
        assert_eq!(preserve.status, 307);
        assert_eq!(
            preserve.location.as_deref(),
            Some("/versioning/redirect-307/result")
        );
        assert_eq!(
            route_request(&request("POST", "/versioning/redirect-307/result", body)).status,
            200
        );
        assert_eq!(
            route_request(&request("GET", "/versioning/redirect-307/result", &[])).status,
            404
        );
    }

    #[test]
    fn versioning_document_counter_records_only_exact_gets() {
        let mut session = versioning_fixtures::SessionState::default();
        let count = || request("GET", "/versioning/document-request-count", &[]);
        assert_eq!(
            versioning_fixtures::route(
                &count(),
                "/versioning/document-request-count",
                &mut session
            )
            .expect("counter route")
            .body,
            b"0"
        );
        assert!(
            versioning_fixtures::route(
                &request("GET", "/versioning/document", &[]),
                "/versioning/document",
                &mut session,
            )
            .is_some()
        );
        assert_eq!(
            versioning_fixtures::route(
                &request("POST", "/versioning/document", &[]),
                "/versioning/document",
                &mut session,
            )
            .expect("versioning fallback")
            .status,
            404
        );
        assert_eq!(
            versioning_fixtures::route(
                &count(),
                "/versioning/document-request-count",
                &mut session
            )
            .expect("counter route")
            .body,
            b"1"
        );
    }

    #[test]
    fn versioning_form_counter_records_only_nonempty_posts() {
        let mut session = versioning_fixtures::SessionState::default();
        let count = || request("GET", "/versioning/form-post-count", &[]);
        assert_eq!(
            versioning_fixtures::route(&count(), "/versioning/form-post-count", &mut session)
                .expect("counter route")
                .body,
            b"0"
        );
        assert_eq!(
            versioning_fixtures::route(
                &request("POST", "/versioning/form/result", b"synthetic=1"),
                "/versioning/form/result",
                &mut session,
            )
            .expect("form result")
            .status,
            200
        );
        assert_eq!(
            versioning_fixtures::route(
                &request("POST", "/versioning/form/result", &[]),
                "/versioning/form/result",
                &mut session,
            )
            .expect("versioning fallback")
            .status,
            404
        );
        assert_eq!(
            versioning_fixtures::route(&count(), "/versioning/form-post-count", &mut session)
                .expect("counter route")
                .body,
            b"1"
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
        assert!(blocking.contains("127.0.0.1:18774/__ferric_browser_blocked__"));
        assert!(blocking.contains("127.0.0.1:18774/__ferric_browser_excepted__"));
        assert!(blocking.contains("__ferric_browser_excepted__"));
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

    #[test]
    fn versioning_pages_are_bounded_and_cover_distinct_behaviours() {
        for (path, marker) in [
            ("/versioning/", "Disposable Versioning fixtures"),
            ("/versioning/document", "id=\"inner\""),
            ("/versioning/form", "autocomplete=\"one-time-code\""),
            ("/versioning/controlled", "model-original"),
            ("/versioning/editor", "contenteditable=\"true\""),
            ("/versioning/media", "preload=\"metadata\""),
            ("/versioning/lifecycle", "addEventListener('freeze'"),
            ("/versioning/mutation", "revision"),
            ("/versioning/frames", "same-origin"),
            ("/versioning/frame-child", "mode: 'closed'"),
            ("/versioning/frame-branch", "frame-branch-left"),
            ("/versioning/frame-branch-left", "branch=left-a"),
            ("/versioning/frame-branch-right", "branch=right-a"),
            (
                "/versioning/frame-branch-leaf?branch=left-a",
                "Finite branch leaf",
            ),
            ("/versioning/activity", "new WebSocket"),
            ("/versioning/archive", "archive-form"),
            ("/versioning/session/login", "account"),
        ] {
            let response = route_request(&request("GET", path, &[]));
            assert_eq!(response.status, 200, "{path}");
            assert!(response.body.len() <= MAX_BODY_BYTES, "{path}");
            let body = String::from_utf8(response.body).expect("UTF-8 fixture");
            assert!(body.contains(marker), "{path} missing {marker}");
            if path == "/versioning/mutation" {
                assert!(body.contains("textContent = String(++revision)"));
                assert!(!body.contains(".value = String(++revision)"));
            }
        }
        for path in [
            "/versioning/activity/poll",
            "/versioning/activity/worker.js",
            "/versioning/activity/sw.js",
            "/versioning/archive.css",
            "/versioning/archive-image",
            "/versioning/archive-frame",
            "/versioning/fault/slow",
            "/versioning/fault/error",
        ] {
            let response = route_request(&request("GET", path, &[]));
            assert!(response.status == 200 || response.status == 503, "{path}");
            assert!(response.body.len() <= MAX_BODY_BYTES, "{path}");
        }
        assert_eq!(
            route_request(&request("GET", "/versioning/unknown", &[])).status,
            404
        );
    }

    #[test]
    fn versioning_form_result_requires_post_without_echoing_inputs() {
        let body = b"password=synthetic-secret&csrf=fixture-only-token";
        let response = route_request(&request("POST", "/versioning/form/result", body));
        assert_eq!(response.status, 200);
        assert!(String::from_utf8_lossy(&response.body).contains("Synthetic POST result"));
        assert!(!String::from_utf8_lossy(&response.body).contains("synthetic-secret"));
        assert_eq!(
            route_request(&request("GET", "/versioning/form/result", &[])).status,
            404
        );
    }

    #[test]
    fn versioning_media_fixture_is_one_second_local_wav() {
        let response = route_request(&request("GET", "/versioning/media-silence.wav", &[]));
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "audio/wav");
        assert_eq!(response.body.len(), 8_044);
        assert_eq!(&response.body[..4], b"RIFF");
        assert_eq!(&response.body[8..12], b"WAVE");
        assert_eq!(&response.body[36..40], b"data");
        assert!(response.body[44..].iter().all(|sample| *sample == 0x80));
        assert_eq!(response.byte_range, Some(ByteRangeResponse::Full));
    }

    #[test]
    fn versioning_media_fixture_supports_single_byte_ranges() {
        let mut request = request("GET", "/versioning/media-silence.wav", &[]);
        request.headers.insert("range".into(), "bytes=0-".into());
        let complete_range = route_request(&request);
        assert_eq!(complete_range.status, 206);
        assert_eq!(complete_range.body.len(), 8_044);
        assert_eq!(
            complete_range.byte_range,
            Some(ByteRangeResponse::Partial {
                start: 0,
                end: 8_043,
                total: 8_044,
            })
        );

        request.headers.insert("range".into(), "bytes=44-47".into());
        let samples = route_request(&request);
        assert_eq!(samples.status, 206);
        assert_eq!(samples.body, vec![0x80; 4]);
        assert_eq!(
            samples.byte_range,
            Some(ByteRangeResponse::Partial {
                start: 44,
                end: 47,
                total: 8_044,
            })
        );

        request.headers.insert("range".into(), "bytes=-4".into());
        let suffix = route_request(&request);
        assert_eq!(suffix.status, 206);
        assert_eq!(suffix.body, vec![0x80; 4]);
        assert_eq!(
            suffix.byte_range,
            Some(ByteRangeResponse::Partial {
                start: 8_040,
                end: 8_043,
                total: 8_044,
            })
        );

        request.headers.insert("range".into(), "bytes=8044-".into());
        let unsatisfiable = route_request(&request);
        assert_eq!(unsatisfiable.status, 416);
        assert!(unsatisfiable.body.is_empty());
        assert_eq!(
            unsatisfiable.byte_range,
            Some(ByteRangeResponse::Unsatisfiable { total: 8_044 })
        );

        request
            .headers
            .insert("range".into(), "bytes=0-1,4-5".into());
        let unsupported_multipart = route_request(&request);
        assert_eq!(unsupported_multipart.status, 200);
        assert_eq!(
            unsupported_multipart.byte_range,
            Some(ByteRangeResponse::Full)
        );
    }

    #[test]
    fn editor_download_arm_applies_to_exactly_one_future_editor_load() {
        let mut state = versioning_fixtures::SessionState::default();
        let editor = request("GET", "/versioning/editor", &[]);
        let plain = route_request_with_state(&editor, &mut state);
        assert!(!String::from_utf8_lossy(&plain.body).contains("slow-download?armed=1"));

        let arm = route_request_with_state(
            &request("POST", "/versioning/editor/arm-download", &[]),
            &mut state,
        );
        assert_eq!(arm.status, 200);
        let armed = route_request_with_state(&editor, &mut state);
        assert!(String::from_utf8_lossy(&armed.body).contains("slow-download?armed=1"));
        let next = route_request_with_state(&editor, &mut state);
        assert!(!String::from_utf8_lossy(&next.body).contains("slow-download?armed=1"));
    }

    #[test]
    fn versioning_cross_origin_links_use_validated_loopback_port() {
        let mut req = request("GET", "/versioning/frames", &[]);
        req.headers.insert("host".into(), "127.0.0.1:18443".into());
        let response = route_request(&req);
        let body = String::from_utf8(response.body).expect("UTF-8 fixture");
        assert!(body.contains("http://localhost:18443/versioning/frame-child"));
        req.headers
            .insert("host".into(), "evil.invalid:bad-port".into());
        let response = route_request(&req);
        let body = String::from_utf8(response.body).expect("UTF-8 fixture");
        assert!(!body.contains("evil.invalid"));
        assert!(body.contains("http://localhost:80/versioning/frame-child"));
    }

    #[test]
    fn versioning_session_requires_cookies_csrf_and_server_state() {
        let mut state = versioning_fixtures::SessionState::default();
        let member = request("GET", "/versioning/session/member", &[]);
        assert_eq!(route_request_with_state(&member, &mut state).status, 404);
        let login = request("POST", "/versioning/session/login", b"account=synthetic");
        let response = route_request_with_state(&login, &mut state);
        assert_eq!(response.status, 303);
        assert_eq!(response.set_cookies.len(), 2);

        let mut member = member;
        member.headers.insert(
            "cookie".into(),
            "vb_session=synthetic; vb_csrf=synthetic-csrf".into(),
        );
        assert_eq!(route_request_with_state(&member, &mut state).status, 200);
        let mut change = request("POST", "/versioning/session/change", b"synthetic-change");
        change.headers = member.headers.clone();
        assert_eq!(route_request_with_state(&change, &mut state).status, 404);
        change
            .headers
            .insert("x-vb-csrf".into(), "synthetic-csrf".into());
        assert_eq!(route_request_with_state(&change, &mut state).status, 200);
        let mut logout = request("POST", "/versioning/session/logout", &[]);
        logout.headers = member.headers.clone();
        let response = route_request_with_state(&logout, &mut state);
        assert_eq!(response.status, 200);
        assert!(
            response
                .set_cookies
                .iter()
                .all(|cookie| cookie.contains("Max-Age=0"))
        );
        assert_eq!(route_request_with_state(&member, &mut state).status, 404);
    }

    #[test]
    fn secure_cookie_route_requires_real_tls_transport_marker() {
        let request = request("GET", "/versioning/cookies/secure", &[]);
        let rejected = route_request(&request);
        assert_eq!(rejected.status, 426);
        assert!(rejected.set_cookies.is_empty());
        let mut secure_request = request;
        secure_request.secure_transport = true;
        let accepted = route_request(&secure_request);
        assert_eq!(accepted.status, 200);
        assert!(
            accepted
                .set_cookies
                .iter()
                .all(|cookie| cookie.contains("Secure"))
        );
        assert!(
            accepted
                .set_cookies
                .iter()
                .any(|cookie| cookie.contains("Partitioned"))
        );
    }

    #[test]
    fn cookie_variant_routes_require_tls_and_scoped_host() {
        let mut req = request("GET", "/versioning/cookies/variants", &[]);
        assert_eq!(route_request(&req).status, 426);
        req.secure_transport = true;
        let variants = route_request(&req);
        assert_eq!(variants.status, 200);
        assert_eq!(variants.set_cookies.len(), 4);
        assert_eq!(
            variants
                .set_cookies
                .iter()
                .filter(|cookie| cookie.starts_with("vb_same="))
                .count(),
            2
        );
        assert!(
            variants
                .set_cookies
                .iter()
                .any(|cookie| cookie.contains("Max-Age=3600"))
        );

        req.target = "/versioning/cookies/domain".into();
        req.headers.insert("host".into(), "127.0.0.1:4443".into());
        assert_eq!(route_request(&req).status, 426);
        req.headers.insert("host".into(), "localhost:4443".into());
        let domain = route_request(&req);
        assert_eq!(domain.status, 200);
        assert_eq!(domain.set_cookies.len(), 1);
        assert!(domain.set_cookies[0].contains("Domain=localhost"));
    }

    #[test]
    fn disposable_http_server_serves_versioning_page_then_exits() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
        let address = listener.local_addr().expect("listener address");
        let server =
            thread::spawn(move || run_http_listener(listener, true, "http", false, false, None));
        let mut stream = TcpStream::connect(address).expect("connect fixture");
        stream
            .write_all(b"GET /versioning/document HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .expect("send request");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("Versioning long document"));
        server.join().expect("server thread").expect("server exit");
    }
}
