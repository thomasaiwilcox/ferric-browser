use std::{
    fmt::Write as _,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use ferric_browser_config::load;

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new(included: &str, root: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "ferric-interaction-sources-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("included.toml"), included).unwrap();
        fs::write(path.join("root.toml"), root).unwrap();
        Self(path)
    }
    fn root(&self) -> PathBuf {
        self.0.join("root.toml")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn parent_array_replacement_cannot_hide_invalid_included_interaction() {
    let fixture = Fixture::new(
        "[[site_rules]]\nid='hidden'\npattern='https://example.com/*'\n[site_rules.interaction]\nenabled=false\nunknown=true\n",
        "schema_version=3\ninclude=['included.toml']\nsite_rules=[]\n",
    );
    let result = load(fixture.root());
    assert!(
        result.is_err(),
        "invalid included data was hidden by replacement"
    );
    assert!(result.unwrap_err().to_string().contains("included.toml"));
}

#[test]
fn unknown_future_sections_reject_in_authored_runtime_and_replaced_includes() {
    use ferric_browser_config::{Config, RuntimeOverrides};
    let version = RuntimeOverrides::default().schema_version;
    for section in ["resources", "permission_policy", "dummy_consumer"] {
        let valid_rule = "[[site_rules]]\nid='future'\npattern='https://example.test/*'\n\
                          [site_rules.interaction]\nenabled=false\n";
        assert!(toml::from_str::<Config>(&format!("schema_version=3\n{valid_rule}")).is_ok());
        assert!(
            toml::from_str::<RuntimeOverrides>(&format!("schema_version={version}\n{valid_rule}"))
                .is_ok()
        );
        let rule = format!(
            "[[site_rules]]\nid='future'\npattern='https://example.test/*'\n\
             [site_rules.interaction]\nenabled=false\n\
             [site_rules.{section}]\nenabled=false\n"
        );
        assert!(toml::from_str::<Config>(&format!("schema_version=3\n{rule}")).is_err());
        assert!(
            toml::from_str::<RuntimeOverrides>(&format!("schema_version={version}\n{rule}"))
                .is_err()
        );
        let fixture = Fixture::new(
            &rule,
            "schema_version=3\ninclude=['included.toml']\nsite_rules=[]\n",
        );
        let error = load(fixture.root()).unwrap_err().to_string();
        assert!(error.contains("included.toml"));
        assert!(error.contains(section));
    }
}

#[test]
fn retained_sources_do_not_resurrect_replaced_rules_or_fill_sparse_defaults() {
    let fixture = Fixture::new(
        "[[site_rules]]\nid='hidden'\npattern='https://example.com/*'\n[site_rules.interaction]\nenabled=false\n[hints]\nchars='abcd'\n[hints.selectors.links]\nmode='replace'\nselectors=['a[']\n",
        "schema_version=3\ninclude=['included.toml']\nsite_rules=[]\n[hints]\nmin_chars=2\n[hints.selectors.links]\nselectors=['a[href]']\n",
    );
    let loaded = load(fixture.root()).unwrap();
    assert!(loaded.config.site_rules.is_empty());
    assert_eq!(loaded.config.hints.chars, "abcd");
    assert_eq!(loaded.config.hints.min_chars, 2);
    let documents = loaded.authored.documents();
    assert_eq!(documents.len(), 2);
    assert_eq!(documents[0].path(), loaded.sources[0]);
    assert_eq!(documents[1].path(), loaded.sources[1]);
    assert_eq!(documents[0].site_rules().unwrap()[0].id, "hidden");
    assert_eq!(documents[1].site_rules(), Some([].as_slice()));
    let child = documents[0].hints().unwrap();
    let parent = documents[1].hints().unwrap();
    assert_eq!(child.chars.as_deref(), Some("abcd"));
    assert_eq!(child.min_chars, None);
    assert_eq!(parent.chars, None);
    assert_eq!(parent.min_chars, Some(2));
    let family = ferric_browser_config::HintSelectorFamily::Links;
    assert_eq!(
        child.selectors.as_ref().unwrap()[&family]
            .selectors
            .as_deref(),
        Some(["a[".into()].as_slice())
    );
    assert_eq!(parent.selectors.as_ref().unwrap()[&family].mode, None);
    assert_eq!(
        loaded.config.hints.selectors[&family].selectors,
        ["a[href]"]
    );
}

#[test]
fn absent_and_empty_source_arrays_are_distinct() {
    let fixture = Fixture::new(
        "",
        "schema_version=3\ninclude=['included.toml']\nsite_rules=[]\n",
    );
    let loaded = load(fixture.root()).unwrap();
    assert_eq!(loaded.authored.documents()[0].site_rules(), None);
    assert_eq!(loaded.authored.documents()[0].hints(), None);
    assert_eq!(
        loaded.authored.documents()[1].site_rules(),
        Some([].as_slice())
    );
}

#[test]
fn registered_globals_keep_explicit_defaults_sparse_tables_and_replaced_arrays() {
    let fixture = Fixture::new(
        "[content]\nzoom=1.0\n[session]\nlazy_restore=false\n\
         [hints.selectors.links]\nmode='replace'\nselectors=['#included']\n\
         [bindings.normal]\nz='quit'\n",
        "schema_version=3\ninclude=['included.toml']\nsite_rules=[]\n\
         [session]\nlazy_restore=true\n[hints.selectors.links]\nselectors=[]\n",
    );
    let loaded = load(fixture.root()).unwrap();
    loaded
        .authored
        .validate_base_settings(&loaded.config)
        .unwrap();
    let child = loaded.authored.documents()[0].settings();
    let parent = loaded.authored.documents()[1].settings();
    assert_eq!(child["content.zoom"].as_float(), Some(1.0));
    assert!(!parent.contains_key("content.zoom"));
    assert!(!child.contains_key("schema_version"));
    assert!(!child.contains_key("content.images"));
    assert_eq!(child["session.lazy_restore"].as_bool(), Some(false));
    assert_eq!(parent["session.lazy_restore"].as_bool(), Some(true));
    assert_eq!(child["bindings"]["normal"]["z"].as_str(), Some("quit"));
    assert!(child["bindings"]["normal"].get("j").is_none());
    let child_selector = &child["hints.selectors.links"];
    let parent_selector = &parent["hints.selectors.links"];
    assert_eq!(child_selector["mode"].as_str(), Some("replace"));
    assert!(parent_selector.get("mode").is_none());
    assert_eq!(parent_selector["selectors"].as_array().unwrap().len(), 0);
    assert!(!parent.contains_key("include"));
    assert!(!parent.contains_key("site_rules"));
    let resolved =
        &loaded.config.hints.selectors[&ferric_browser_config::HintSelectorFamily::Links];
    assert_eq!(
        resolved.mode,
        ferric_browser_config::HintSelectorMode::Replace
    );
    assert!(resolved.selectors.is_empty());
}

#[test]
fn failed_reload_keeps_effective_config_and_authored_catalogue_together() {
    let fixture = Fixture::new(
        "[hints]\nchars='abcd'\n",
        "schema_version=3\ninclude=['included.toml']\nsite_rules=[]\n",
    );
    let mut store = ferric_browser_config::ConfigStore::new();
    let previous = store.reload(fixture.root()).unwrap().clone();
    let revision = store.revision();
    fs::write(fixture.0.join("included.toml"), "[[site_rules]]\nid='broken'\npattern='https://example.com/*'\n[site_rules.interaction]\nenabled=false\nunknown=true\n").unwrap();
    assert!(store.reload(fixture.root()).is_err());
    assert_eq!(store.revision(), revision);
    assert_eq!(store.current(), Some(&previous));
    assert!(std::sync::Arc::ptr_eq(
        &store.current().unwrap().authored,
        &previous.authored
    ));
}

#[test]
fn replacement_cannot_hide_duplicate_ids_or_invalid_raw_selector_shape() {
    for included in [
        "[[site_rules]]\nid='dup'\npattern='https://example.com/*'\n[[site_rules]]\nid='dup'\npattern='https://other.test/*'\n",
        "[hints.selectors.links]\nselectors=['']\n",
        "[[site_rules]]\nid='hidden'\npattern='https://example.com/*'\n[site_rules.interaction]\nenabled=false\nlabels={min_chars=0}\n",
    ] {
        let fixture = Fixture::new(
            included,
            "schema_version=3\ninclude=['included.toml']\nsite_rules=[]\n[hints.selectors.links]\nmode='replace'\nselectors=[]\n",
        );
        assert!(
            load(fixture.root()).is_err(),
            "hidden invalid source: {included}"
        );
    }
}

#[test]
fn hidden_shared_selectors_keep_the_existing_aggregate_budget() {
    for family_count in [2, 3] {
        let selectors: Vec<_> = (0..16)
            .map(|index| format!("a{index:02}{}", "x".repeat(1021)))
            .collect();
        let list = toml::Value::Array(selectors.into_iter().map(toml::Value::String).collect());
        let mut included = String::new();
        let mut root = "schema_version=3\ninclude=['included.toml']\n".to_owned();
        for family in ["links", "buttons", "images"]
            .into_iter()
            .take(family_count)
        {
            write!(
                included,
                "[hints.selectors.{family}]\nmode='replace'\nselectors={list}\n"
            )
            .unwrap();
            write!(
                root,
                "[hints.selectors.{family}]\nmode='replace'\nselectors=[]\n"
            )
            .unwrap();
        }
        let fixture = Fixture::new(&included, &root);
        let result = load(fixture.root());
        if family_count == 2 {
            assert!(result.is_ok(), "exact 32 KiB authored budget: {result:?}");
        } else {
            assert!(result.unwrap_err().to_string().contains("32 KiB"));
        }
    }
}

#[test]
fn registered_catalogue_rejects_another_base_and_preserves_numeric_normalization() {
    let fixture = Fixture::new(
        "[ui]\nfont_size_pt=18\n[content]\nzoom=1\n",
        "schema_version=3\ninclude=['included.toml']\n",
    );
    let loaded = load(fixture.root()).unwrap();
    loaded
        .authored
        .validate_base_settings(&loaded.config)
        .unwrap();
    let mut other = loaded.config.clone();
    other.ui.font_size_pt = 20.0;
    let error = loaded
        .authored
        .validate_base_settings(&other)
        .unwrap_err()
        .to_string();
    assert!(error.contains("ui.font_size_pt"));
    let mut other = loaded.config.clone();
    other.content.images = !other.content.images;
    assert!(loaded.authored.validate_base_settings(&other).is_err());
}
