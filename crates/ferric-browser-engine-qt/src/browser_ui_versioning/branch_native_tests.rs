use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use cxx_qt_lib::{
    QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QString, QUrl,
    QVariant,
};
use ferric_browser_config::{Config, ContextsConfig, RuntimeOverrides};
use uuid::Uuid;

struct OneUsePageServer {
    url: String,
    proof_url: String,
    popup_url: String,
    blank_url: String,
    permission_url: String,
    mixed_url: String,
    origin: String,
    gets: Arc<AtomicUsize>,
    parent_cookie_gets: Arc<AtomicUsize>,
    rejected_document_gets: Arc<AtomicUsize>,
    popup_gets: Arc<AtomicUsize>,
    oauth_gets: Arc<AtomicUsize>,
    redirect_final_gets: Arc<AtomicUsize>,
    mixed_gets: Arc<AtomicUsize>,
    reject_uncookied: Arc<AtomicBool>,
    hold_uncookied_document: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl OneUsePageServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let address = listener.local_addr().expect("loopback address");
        let gets = Arc::new(AtomicUsize::new(0));
        let parent_cookie_gets = Arc::new(AtomicUsize::new(0));
        let rejected_document_gets = Arc::new(AtomicUsize::new(0));
        let popup_gets = Arc::new(AtomicUsize::new(0));
        let oauth_gets = Arc::new(AtomicUsize::new(0));
        let redirect_final_gets = Arc::new(AtomicUsize::new(0));
        let mixed_gets = Arc::new(AtomicUsize::new(0));
        let reject_uncookied = Arc::new(AtomicBool::new(false));
        let hold_uncookied_document = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_gets = Arc::clone(&gets);
        let worker_parent_cookie_gets = Arc::clone(&parent_cookie_gets);
        let worker_rejected_document_gets = Arc::clone(&rejected_document_gets);
        let worker_popup_gets = Arc::clone(&popup_gets);
        let worker_oauth_gets = Arc::clone(&oauth_gets);
        let worker_redirect_final_gets = Arc::clone(&redirect_final_gets);
        let worker_mixed_gets = Arc::clone(&mixed_gets);
        let worker_reject_uncookied = Arc::clone(&reject_uncookied);
        let worker_hold_uncookied_document = Arc::clone(&hold_uncookied_document);
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut request = [0_u8; 4096];
                let length = stream.read(&mut request).unwrap_or(0);
                let first_line = request[..length].split(|byte| *byte == b'\n').next();
                let has_parent_cookie = request[..length]
                    .windows(b"ferric_branch_source=1".len())
                    .any(|part| part == b"ferric_branch_source=1");
                if first_line.is_some_and(|line| line.starts_with(b"GET /document")) {
                    worker_gets.fetch_add(1, Ordering::Relaxed);
                    if has_parent_cookie {
                        worker_parent_cookie_gets.fetch_add(1, Ordering::Relaxed);
                    }
                    if has_parent_cookie || !worker_reject_uncookied.load(Ordering::Relaxed) {
                        let body =
                            b"<!doctype html><title>One-use branch page</title><p>synthetic</p>";
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nSet-Cookie: ferric_branch_source=1; Path=/; SameSite=Lax\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(body);
                    } else {
                        worker_rejected_document_gets.fetch_add(1, Ordering::Relaxed);
                        let hold_deadline = Instant::now() + Duration::from_secs(25);
                        while worker_hold_uncookied_document.load(Ordering::Relaxed)
                            && !worker_stop.load(Ordering::Relaxed)
                            && Instant::now() < hold_deadline
                        {
                            thread::sleep(Duration::from_millis(10));
                        }
                    }
                    // Once the parent has proven its cookie, only an uncookied
                    // clean-child replay is closed without a response.
                } else if first_line.is_some_and(|line| line.starts_with(b"GET /popup ")) {
                    worker_popup_gets.fetch_add(1, Ordering::Relaxed);
                    let body = b"<!doctype html><title>Branch popup page</title><button onclick=\"window.open('/popup-target', 'fixture')\">open popup</button>";
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(body);
                } else if first_line.is_some_and(|line| line.starts_with(b"GET /popup-target ")) {
                    let header = format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://localhost:{}/oauth-idp\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        address.port()
                    );
                    let _ = stream.write_all(header.as_bytes());
                } else if first_line.is_some_and(|line| line.starts_with(b"GET /oauth-idp ")) {
                    worker_oauth_gets.fetch_add(1, Ordering::Relaxed);
                    let header = format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{}/popup-final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        address.port()
                    );
                    let _ = stream.write_all(header.as_bytes());
                } else if first_line.is_some_and(|line| line.starts_with(b"GET /popup-final ")) {
                    worker_redirect_final_gets.fetch_add(1, Ordering::Relaxed);
                    let body = b"<!doctype html><title>Branch popup final</title>synthetic";
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(body);
                } else if first_line.is_some_and(|line| line.starts_with(b"GET /blank ")) {
                    let body = b"<!doctype html><title>Branch target blank page</title><a href='/popup-target' target='_blank'>open popup</a>";
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(body);
                } else if first_line.is_some_and(|line| line.starts_with(b"GET /permission ")) {
                    let body = b"<!doctype html><title>Branch permission page</title><button onclick='Notification.requestPermission()'>request permission</button>";
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(body);
                } else if first_line.is_some_and(|line| line.starts_with(b"GET /mixed ")) {
                    worker_mixed_gets.fetch_add(1, Ordering::Relaxed);
                    if has_parent_cookie || !worker_reject_uncookied.load(Ordering::Relaxed) {
                        let body = b"<!doctype html><title>One-use mixed page</title>synthetic";
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(body);
                    }
                    // The uncookied child's deferred replay closes without a response.
                } else {
                    let _ = stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                }
            }
        });
        Self {
            url: format!("http://{address}/document"),
            proof_url: format!("http://{address}/document?proof=typed-get"),
            popup_url: format!("http://{address}/popup"),
            blank_url: format!("http://{address}/blank"),
            permission_url: format!("http://{address}/permission"),
            mixed_url: format!("http://{address}/mixed"),
            origin: format!("http://{address}"),
            gets,
            parent_cookie_gets,
            rejected_document_gets,
            popup_gets,
            oauth_gets,
            redirect_final_gets,
            mixed_gets,
            reject_uncookied,
            hold_uncookied_document,
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for OneUsePageServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.join().expect("loopback worker");
        }
    }
}

struct DisposableProfile(PathBuf);

impl Drop for DisposableProfile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn native_clean_window_branch_failure_close_and_popup_isolation() {
    let fixture = OneUsePageServer::start();
    let base = DisposableProfile(
        std::env::temp_dir().join(format!("ferric-clean-branch-native-{}", Uuid::new_v4())),
    );
    fs::create_dir(&base.0).expect("disposable profile root");
    crate::register_internal_scheme();
    cxx_qt::init_crate!(ferric_browser_engine_qt);
    cxx_qt::init_qml_module!("io.github.ferricbrowser");
    let app = QGuiApplication::new();
    assert!(app.as_ref().is_some(), "Qt GUI application");
    let mut engine = QQmlApplicationEngine::new();
    let config = serde_json::to_string(&Config::default()).expect("default config");
    let contexts = serde_json::to_string(&ContextsConfig::default()).expect("contexts");
    let overrides = serde_json::to_string(&RuntimeOverrides::default()).expect("overrides");
    let mut properties: QMap<QMapPair_QString_QVariant> = QMap::default();
    for (name, value) in [
        ("startupUrl", fixture.url.as_str()),
        ("startupConfigJson", config.as_str()),
        ("startupConfigBaseJson", config.as_str()),
        ("startupContextsJson", contexts.as_str()),
        ("startupCliOverridesJson", overrides.as_str()),
        ("startupProfileOverridesJson", overrides.as_str()),
        (
            "storageBasePath",
            base.0.to_str().expect("UTF-8 disposable path"),
        ),
    ] {
        properties.insert(QString::from(name), QVariant::from(&QString::from(value)));
    }
    properties.insert(
        QString::from("instanceRequestOwner"),
        QVariant::from(&false),
    );
    engine
        .as_mut()
        .expect("QML engine")
        .set_initial_properties(&properties);
    engine.as_mut().expect("QML engine").load(&QUrl::from(
        "qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml",
    ));

    let source_state = |engine: &mut cxx::UniquePtr<QQmlApplicationEngine>| {
        let json = crate::qobject::ferric_browser_test_page_capture_state(
            engine.as_mut().expect("QML engine"),
        );
        serde_json::from_str::<serde_json::Value>(&json.to_string()).expect("source state")
    };
    let branch_state = |engine: &mut cxx::UniquePtr<QQmlApplicationEngine>| {
        let json = crate::qobject::ferric_browser_test_clean_branch_state(
            engine.as_mut().expect("QML engine"),
        );
        serde_json::from_str::<serde_json::Value>(&json.to_string()).expect("branch state")
    };
    let wait_until =
        |engine: &mut cxx::UniquePtr<QQmlApplicationEngine>,
         predicate: &dyn Fn(&serde_json::Value, &serde_json::Value) -> bool| {
            let deadline = Instant::now() + Duration::from_secs(25);
            loop {
                app.as_ref().expect("Qt GUI application").process_events();
                let source = source_state(engine);
                let branch = branch_state(engine);
                if predicate(&source, &branch) {
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "native branch state did not converge: document_GETs={} parent_cookie_GETs={} rejected_document_GETs={} mixed_GETs={} oauth_GETs={} source={source} branch={branch}",
                    fixture.gets.load(Ordering::Relaxed),
                    fixture.parent_cookie_gets.load(Ordering::Relaxed),
                    fixture.rejected_document_gets.load(Ordering::Relaxed),
                    fixture.mixed_gets.load(Ordering::Relaxed),
                    fixture.oauth_gets.load(Ordering::Relaxed)
                );
                thread::sleep(Duration::from_millis(10));
            }
        };
    let finish_child_close = |engine: &mut cxx::UniquePtr<QQmlApplicationEngine>| {
        for _ in 0..5 {
            wait_until(engine, &|_, branch| {
                branch["child_present"] == false || branch["page_prompt"] == true
            });
            if branch_state(engine)["child_present"] == false {
                return;
            }
            assert!(
                crate::qobject::ferric_browser_test_confirm_clean_branch_page_close(
                    engine.as_mut().expect("QML engine"),
                )
            );
        }
        wait_until(engine, &|_, branch| branch["child_present"] == false);
    };

    wait_until(&mut engine, &|source, _| {
        source["ui_ready"] == true
            && source["profile_bootstrap_complete"] == true
            && source["view_ready"] == true
    });
    // Startup may fetch the fixture and still settle on the earlier blank
    // view. Require a committed, cookie-bearing source page before taking
    // any safe-GET proof. The later distinct typed URL is the branch source.
    for _ in 0..5 {
        let source = source_state(&mut engine);
        if source["current_url"] == fixture.url
            && source["view_url"] == fixture.url
            && source["load_state"] == "complete"
            && fixture.parent_cookie_gets.load(Ordering::Relaxed) > 0
        {
            break;
        }
        let command = if source["view_url"] == fixture.url {
            QString::from("reload")
        } else {
            QString::from(format!("open {}", fixture.url))
        };
        assert!(crate::qobject::ferric_browser_test_execute_root_command(
            engine.as_mut().expect("QML engine"),
            &command,
        ));
        let reload_deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < reload_deadline {
            app.as_ref().expect("Qt GUI application").process_events();
            let source = source_state(&mut engine);
            if source["current_url"] == fixture.url
                && source["view_url"] == fixture.url
                && source["load_state"] == "complete"
                && fixture.parent_cookie_gets.load(Ordering::Relaxed) > 0
            {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
    wait_until(&mut engine, &|source, _| {
        source["current_url"] == fixture.url
            && source["load_state"] == "complete"
            && fixture.parent_cookie_gets.load(Ordering::Relaxed) > 0
    });
    let parent_cookie_gets_before_proof = fixture.parent_cookie_gets.load(Ordering::Relaxed);
    let open_proof = QString::from(format!("open {}", fixture.proof_url));
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &open_proof,
    ));
    wait_until(&mut engine, &|source, _| {
        source["current_url"] == fixture.proof_url
            && source["view_url"] == fixture.proof_url
            && source["load_state"] == "complete"
            && fixture.parent_cookie_gets.load(Ordering::Relaxed) > parent_cookie_gets_before_proof
    });
    fixture.reject_uncookied.store(true, Ordering::Relaxed);
    fixture
        .hold_uncookied_document
        .store(true, Ordering::Relaxed);
    let rejected_document_gets_before_branch =
        fixture.rejected_document_gets.load(Ordering::Relaxed);

    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &QString::from("branch vb061-native-failure --clean --scope=window"),
    ));
    wait_until(&mut engine, &|source, branch| {
        source["current_url"] == fixture.proof_url
            && branch["child_published"] == true
            && branch["child_ephemeral"] == true
            && branch["child_tab_count"] == 1
            && branch["planned_starts"][0] == true
            && branch["outcomes"][0] == "failed"
            && branch["child_status"] == "Clean branch page load timed out"
            && fixture.rejected_document_gets.load(Ordering::Relaxed)
                > rejected_document_gets_before_branch
    });
    fixture
        .hold_uncookied_document
        .store(false, Ordering::Relaxed);

    assert!(
        crate::qobject::ferric_browser_test_request_clean_branch_close(
            engine.as_mut().expect("QML engine"),
        )
    );
    wait_until(&mut engine, &|_, branch| branch["close_prompt"] == true);
    assert!(
        crate::qobject::ferric_browser_test_confirm_clean_branch_close(
            engine.as_mut().expect("QML engine"),
        )
    );
    finish_child_close(&mut engine);
    wait_until(&mut engine, &|source, branch| {
        source["root_ready"] == true && branch["child_present"] == false
    });
    let open_popup_source = QString::from(format!("open {}", fixture.popup_url));
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &open_popup_source,
    ));
    wait_until(&mut engine, &|source, _| {
        source["current_url"] == fixture.popup_url
            && source["view_url"] == fixture.popup_url
            && source["load_state"] == "complete"
            && fixture.popup_gets.load(Ordering::Relaxed) >= 1
    });
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &QString::from("branch vb063-native-popup --clean --scope=window"),
    ));
    wait_until(&mut engine, &|_, branch| {
        branch["child_published"] == true && branch["child_ephemeral"] == true
    });
    assert!(
        crate::qobject::ferric_browser_test_execute_clean_branch_command(
            engine.as_mut().expect("QML engine"),
            &QString::from(format!("open {}", fixture.popup_url)),
        )
    );
    wait_until(&mut engine, &|_, branch| {
        branch["child_current_url"] == fixture.popup_url
            && branch["child_view_url"] == fixture.popup_url
            && branch["child_load_state"] == "complete"
            && fixture.popup_gets.load(Ordering::Relaxed) >= 2
    });
    assert!(crate::qobject::ferric_browser_test_click_clean_branch_page(
        engine.as_mut().expect("QML engine"),
    ));
    wait_until(&mut engine, &|_, branch| {
        branch["popup_count"]
            .as_i64()
            .is_some_and(|count| count >= 1)
            && branch["popup_owners_exact"] == true
            && fixture.oauth_gets.load(Ordering::Relaxed) >= 1
            && fixture.redirect_final_gets.load(Ordering::Relaxed) >= 1
    });
    assert!(
        crate::qobject::ferric_browser_test_request_clean_branch_close(
            engine.as_mut().expect("QML engine"),
        )
    );
    wait_until(&mut engine, &|_, branch| branch["close_prompt"] == true);
    assert!(
        crate::qobject::ferric_browser_test_confirm_clean_branch_close(
            engine.as_mut().expect("QML engine"),
        )
    );
    wait_until(&mut engine, &|_, branch| {
        branch["all_branch_popup_count"] == 0 || branch["popup_states"][0]["page_prompt"] == true
    });
    if branch_state(&mut engine)["all_branch_popup_count"] != 0 {
        assert!(
            crate::qobject::ferric_browser_test_confirm_clean_branch_popup_page_close(
                engine.as_mut().expect("QML engine"),
            )
        );
    }
    finish_child_close(&mut engine);
    wait_until(&mut engine, &|source, branch| {
        source["root_ready"] == true
            && branch["child_present"] == false
            && branch["all_branch_popup_count"] == 0
    });
    let open_mixed = QString::from(format!("tab-open {}", fixture.mixed_url));
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &open_mixed,
    ));
    wait_until(&mut engine, &|source, _| {
        source["tab_count"] == 2
            && source["current_url"] == fixture.mixed_url
            && source["load_state"] == "complete"
            && fixture.mixed_gets.load(Ordering::Relaxed) >= 1
    });
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &QString::from("tab-open about:blank"),
    ));
    wait_until(&mut engine, &|source, _| {
        source["tab_count"] == 3
            && source["active_tab_index"] == 2
            && source["current_url"] == "about:blank"
            && source["load_state"] == "complete"
    });
    let mixed_gets_before_branch = fixture.mixed_gets.load(Ordering::Relaxed);
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &QString::from("branch vb061-native-mixed --clean --scope=window"),
    ));
    wait_until(&mut engine, &|source, branch| {
        source["tab_count"] == 3
            && source["active_tab_index"] == 2
            && branch["child_published"] == true
            && branch["child_tab_count"] == 3
            && branch["outcomes"][0] == "pending"
            && branch["outcomes"][1] == "pending"
            && fixture.mixed_gets.load(Ordering::Relaxed) == mixed_gets_before_branch
    });
    assert!(
        crate::qobject::ferric_browser_test_execute_clean_branch_command(
            engine.as_mut().expect("QML engine"),
            &QString::from("tab-select 2"),
        )
    );
    wait_until(&mut engine, &|source, branch| {
        source["tab_count"] == 3
            && branch["outcomes"][1] == "failed"
            && fixture.mixed_gets.load(Ordering::Relaxed) > mixed_gets_before_branch
    });
    let original_tab_ids = branch_state(&mut engine)["child_tab_ids"]
        .as_array()
        .expect("three child tab IDs")
        .clone();
    assert_eq!(original_tab_ids.len(), 3);
    assert!(
        crate::qobject::ferric_browser_test_execute_clean_branch_command(
            engine.as_mut().expect("QML engine"),
            &QString::from("tab-move right"),
        )
    );
    wait_until(&mut engine, &|source, branch| {
        source["tab_count"] == 3
            && branch["child_tab_ids"][0] == original_tab_ids[0]
            && branch["child_tab_ids"][1] == original_tab_ids[2]
            && branch["child_tab_ids"][2] == original_tab_ids[1]
            && branch["outcomes"][1] == "failed"
    });
    assert!(
        crate::qobject::ferric_browser_test_execute_clean_branch_command(
            engine.as_mut().expect("QML engine"),
            &QString::from("tab-select 1"),
        )
    );
    wait_until(&mut engine, &|source, branch| {
        source["tab_count"] == 3
            && branch["outcomes"][0] == "loaded"
            && branch["outcomes"][1] == "failed"
            && branch["outcomes"][2] == "pending"
    });
    assert!(
        crate::qobject::ferric_browser_test_execute_clean_branch_command(
            engine.as_mut().expect("QML engine"),
            &QString::from("tab-close"),
        )
    );
    wait_until(&mut engine, &|source, branch| {
        source["tab_count"] == 3
            && branch["child_tab_count"] == 3
            && branch["child_tab_ids"][0] == original_tab_ids[0]
            && branch["child_tab_ids"][1] == original_tab_ids[2]
            && branch["child_tab_ids"][2] == original_tab_ids[1]
            && branch["child_status"] == "No unpinned tabs available to close"
    });
    assert!(
        crate::qobject::ferric_browser_test_first_clean_branch_tab_close_refused(
            engine.as_mut().expect("QML engine"),
        )
    );
    wait_until(&mut engine, &|source, branch| {
        source["tab_count"] == 3
            && branch["child_tab_count"] == 3
            && branch["child_tab_ids"][0] == original_tab_ids[0]
            && branch["child_tab_ids"][1] == original_tab_ids[2]
            && branch["child_tab_ids"][2] == original_tab_ids[1]
            && branch["child_status"]
                == "The first clean-branch tab anchors its profile; close the branch window"
    });
    assert!(
        crate::qobject::ferric_browser_test_request_clean_branch_close(
            engine.as_mut().expect("QML engine"),
        )
    );
    wait_until(&mut engine, &|_, branch| branch["close_prompt"] == true);
    assert!(
        crate::qobject::ferric_browser_test_confirm_clean_branch_close(
            engine.as_mut().expect("QML engine"),
        )
    );
    finish_child_close(&mut engine);
    wait_until(&mut engine, &|source, branch| {
        source["tab_count"] == 3 && branch["child_present"] == false
    });
    for expected_tabs in [2, 1] {
        assert!(crate::qobject::ferric_browser_test_execute_root_command(
            engine.as_mut().expect("QML engine"),
            &QString::from("tab-close"),
        ));
        wait_until(&mut engine, &|source, _| {
            source["tab_count"] == expected_tabs
        });
    }
    let open_permission_source = QString::from(format!("open {}", fixture.permission_url));
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &open_permission_source,
    ));
    wait_until(&mut engine, &|source, _| {
        source["current_url"] == fixture.permission_url
            && source["view_url"] == fixture.permission_url
            && source["load_state"] == "complete"
    });
    assert!(
        crate::qobject::ferric_browser_test_remember_normal_notification_permission(
            engine.as_mut().expect("QML engine"),
            &QString::from(fixture.origin.as_str()),
        )
    );
    let permission_deadline = Instant::now() + Duration::from_secs(25);
    while !crate::qobject::ferric_browser_test_normal_notification_allowed(
        engine.as_mut().expect("QML engine"),
        &QString::from(fixture.origin.as_str()),
    ) {
        app.as_ref().expect("Qt GUI application").process_events();
        assert!(
            Instant::now() < permission_deadline,
            "normal-site allow did not commit"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &QString::from("branch vb063-native-permission --clean --scope=window"),
    ));
    wait_until(&mut engine, &|_, branch| {
        branch["child_published"] == true && branch["child_ephemeral"] == true
    });
    assert!(
        crate::qobject::ferric_browser_test_execute_clean_branch_command(
            engine.as_mut().expect("QML engine"),
            &QString::from(format!("open {}", fixture.permission_url)),
        )
    );
    wait_until(&mut engine, &|_, branch| {
        branch["child_current_url"] == fixture.permission_url
            && branch["child_view_url"] == fixture.permission_url
            && branch["child_load_state"] == "complete"
    });
    assert!(crate::qobject::ferric_browser_test_click_clean_branch_page(
        engine.as_mut().expect("QML engine"),
    ));
    wait_until(&mut engine, &|_, branch| {
        branch["child_permission_prompt"] == true
            && branch["child_permission_name"] == "notifications"
            && branch["child_permission_owner_exact"] == true
            && branch["parent_permission_prompt"] == false
    });
    assert!(
        crate::qobject::ferric_browser_test_deny_clean_branch_permission(
            engine.as_mut().expect("QML engine"),
        )
    );
    wait_until(&mut engine, &|_, branch| {
        branch["child_permission_prompt"] == false
    });
    assert!(
        crate::qobject::ferric_browser_test_request_clean_branch_close(
            engine.as_mut().expect("QML engine"),
        )
    );
    wait_until(&mut engine, &|_, branch| branch["close_prompt"] == true);
    assert!(
        crate::qobject::ferric_browser_test_confirm_clean_branch_close(
            engine.as_mut().expect("QML engine"),
        )
    );
    finish_child_close(&mut engine);
    wait_until(&mut engine, &|source, branch| {
        source["root_ready"] == true && branch["child_present"] == false
    });
    let open_blank_source = QString::from(format!("open {}", fixture.blank_url));
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &open_blank_source,
    ));
    wait_until(&mut engine, &|source, _| {
        source["current_url"] == fixture.blank_url
            && source["view_url"] == fixture.blank_url
            && source["load_state"] == "complete"
    });
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &QString::from("branch vb063-native-target-blank --clean --scope=window"),
    ));
    wait_until(&mut engine, &|_, branch| {
        branch["child_published"] == true && branch["child_ephemeral"] == true
    });
    assert!(
        crate::qobject::ferric_browser_test_execute_clean_branch_command(
            engine.as_mut().expect("QML engine"),
            &QString::from(format!("open {}", fixture.blank_url)),
        )
    );
    wait_until(&mut engine, &|_, branch| {
        branch["child_current_url"] == fixture.blank_url
            && branch["child_view_url"] == fixture.blank_url
            && branch["child_load_state"] == "complete"
    });
    assert!(crate::qobject::ferric_browser_test_click_clean_branch_page(
        engine.as_mut().expect("QML engine"),
    ));
    wait_until(&mut engine, &|_, branch| {
        branch["popup_count"] == 1
            && branch["popup_owners_exact"] == true
            && fixture.oauth_gets.load(Ordering::Relaxed) >= 2
            && fixture.redirect_final_gets.load(Ordering::Relaxed) >= 2
    });
    assert!(
        crate::qobject::ferric_browser_test_request_clean_branch_close(
            engine.as_mut().expect("QML engine"),
        )
    );
    wait_until(&mut engine, &|_, branch| branch["close_prompt"] == true);
    assert!(
        crate::qobject::ferric_browser_test_confirm_clean_branch_close(
            engine.as_mut().expect("QML engine"),
        )
    );
    wait_until(&mut engine, &|_, branch| {
        branch["all_branch_popup_count"] == 0 || branch["popup_states"][0]["page_prompt"] == true
    });
    if branch_state(&mut engine)["all_branch_popup_count"] != 0 {
        assert!(
            crate::qobject::ferric_browser_test_confirm_clean_branch_popup_page_close(
                engine.as_mut().expect("QML engine"),
            )
        );
    }
    finish_child_close(&mut engine);
    wait_until(&mut engine, &|source, branch| {
        source["root_ready"] == true
            && branch["child_present"] == false
            && branch["all_branch_popup_count"] == 0
    });
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &QString::from("tab-open about:blank"),
    ));
    wait_until(&mut engine, &|source, _| {
        source["tab_count"] == 2
            && source["active_tab_index"] == 1
            && source["current_url"] == "about:blank"
    });
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &QString::from("branch vb061-native-view-failure --clean --scope=window"),
    ));
    let allocation_deadline = Instant::now() + Duration::from_secs(25);
    loop {
        app.as_ref().expect("Qt GUI application").process_events();
        let branch = branch_state(&mut engine);
        assert!(
            branch["child_published"] == false,
            "child published before view-failure injection: {branch}"
        );
        if branch["child_present"] == true && branch["child_native_bound"] == true {
            assert!(
                crate::qobject::ferric_browser_test_disable_pending_clean_branch_view_factory(
                    engine.as_mut().expect("QML engine"),
                )
            );
            break;
        }
        assert!(
            Instant::now() < allocation_deadline,
            "hidden clean-branch child was not created"
        );
        thread::sleep(Duration::from_millis(1));
    }
    wait_until(&mut engine, &|source, branch| {
        source["tab_count"] == 2
            && source["current_url"] == "about:blank"
            && branch["child_present"] == false
    });
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &QString::from("branch vb061-native-after-view-failure --clean --scope=window"),
    ));
    wait_until(&mut engine, &|source, branch| {
        source["tab_count"] == 2
            && branch["child_published"] == true
            && branch["child_ephemeral"] == true
            && branch["child_tab_count"] == 2
    });
    assert!(
        crate::qobject::ferric_browser_test_request_clean_branch_close(
            engine.as_mut().expect("QML engine"),
        )
    );
    wait_until(&mut engine, &|_, branch| branch["close_prompt"] == true);
    assert!(
        crate::qobject::ferric_browser_test_confirm_clean_branch_close(
            engine.as_mut().expect("QML engine"),
        )
    );
    finish_child_close(&mut engine);
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &QString::from("tab-close"),
    ));
    wait_until(&mut engine, &|source, branch| {
        source["tab_count"] == 1 && branch["child_present"] == false
    });
    assert!(
        crate::qobject::ferric_browser_test_disable_clean_branch_factory(
            engine.as_mut().expect("QML engine"),
        )
    );
    assert!(crate::qobject::ferric_browser_test_execute_root_command(
        engine.as_mut().expect("QML engine"),
        &QString::from("branch vb061-native-no-factory --clean --scope=window"),
    ));
    wait_until(&mut engine, &|source, branch| {
        source["tab_count"] == 1
            && source["status"] == "Clean window branch request or host unavailable"
            && branch["child_present"] == false
    });
    drop(engine);
    drop(app);
    fs::remove_dir_all(&base.0).expect("remove exact disposable profile root");
}
