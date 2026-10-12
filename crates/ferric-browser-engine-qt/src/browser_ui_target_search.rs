use super::{CommandRegistry, CxxQtType, ParsedCommand, Pin, QString, current_target, qobject};
use ferric_browser_core::HintKind;

pub(super) fn target_search_scope(arguments: &[String]) -> Result<&str, String> {
    match arguments {
        [] => Ok("all"),
        [scope]
            if matches!(
                scope.as_str(),
                "links" | "controls" | "regions" | "surfaces" | "all"
            ) =>
        {
            Ok(scope)
        }
        [_] => Err("target-search scope must be links, controls, regions, surfaces, or all".into()),
        _ => Err("target-search accepts at most one scope and no query text".into()),
    }
}

pub(super) fn target_search_collection_family(scope: &str) -> Option<&'static str> {
    match scope {
        "links" => Some("links"),
        "regions" => Some("regions"),
        "surfaces" => Some("surfaces"),
        "controls" | "all" => Some("all"),
        _ => None,
    }
}

pub(super) fn target_search_candidate_matches(scope: &str, kind: HintKind) -> bool {
    match scope {
        "links" => kind == HintKind::Link,
        "controls" => matches!(
            kind,
            HintKind::Button
                | HintKind::Aria
                | HintKind::Input
                | HintKind::Select
                | HintKind::Textarea
                | HintKind::ContentEditable
                | HintKind::EditorFocus
                | HintKind::LabelControl
        ),
        "regions" => matches!(kind, HintKind::Region | HintKind::ScrollableRegion),
        "surfaces" => matches!(kind, HintKind::Surface | HintKind::FocusableSurface),
        "all" => true,
        _ => false,
    }
}

impl qobject::BrowserUi {
    pub(super) fn dispatch_target_search_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        registry: &CommandRegistry,
    ) -> bool {
        if let Err(error) = registry.validate(command, self.as_ref().rust().core_mode) {
            self.set_status_text(QString::from(error.to_string()));
            return false;
        }
        let scope = match target_search_scope(&command.arguments) {
            Ok(scope) => scope,
            Err(error) => {
                self.set_status_text(QString::from(error));
                return false;
            }
        };
        let Some(family) = target_search_collection_family(scope) else {
            self.set_status_text(QString::from("Target Search scope is unsupported"));
            return false;
        };
        let can_dispatch = {
            let pinned = self.as_ref();
            let rust = pinned.rust();
            !rust.hint_pointer_mode
                && !rust.hint_pointer_pending
                && rust.recording_macro.is_none()
                && rust.macro_depth == 0
                && current_target(rust.state.as_ref(), rust.tab).is_some()
        };
        if !can_dispatch {
            self.set_status_text(QString::from(
                "Target Search requires an available page and direct local input",
            ));
            return false;
        }

        self.as_mut().set_target_search_query(QString::default());
        self.as_mut()
            .set_hint_options(family, false, "current", None, false, 1);
        self.as_mut().set_target_search_scope(QString::from(scope));
        self.as_mut().set_target_search_active(true);
        self.set_status_text(QString::from(format!(
            "Search visible page targets · {scope}"
        )));
        true
    }

    pub(super) fn cancel_target_search(mut self: Pin<&mut Self>) {
        if self.as_ref().rust().target_search_active {
            self.as_mut().cancel_hints();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        target_search_candidate_matches, target_search_collection_family, target_search_scope,
    };
    use ferric_browser_core::HintKind;

    #[test]
    fn target_search_grammar_is_closed_and_has_no_query_argument() {
        assert_eq!(target_search_scope(&[]), Ok("all"));
        for scope in ["links", "controls", "regions", "surfaces", "all"] {
            assert_eq!(target_search_scope(&[scope.into()]), Ok(scope));
        }
        assert_eq!(
            target_search_scope(&["needle".into()]),
            Err("target-search scope must be links, controls, regions, surfaces, or all".into())
        );
        assert_eq!(
            target_search_scope(&["links".into(), "needle".into()]),
            Err("target-search accepts at most one scope and no query text".into())
        );
    }

    #[test]
    fn target_search_uses_existing_collector_families_and_closed_kind_filters() {
        assert_eq!(target_search_collection_family("links"), Some("links"));
        assert_eq!(target_search_collection_family("controls"), Some("all"));
        assert_eq!(target_search_collection_family("regions"), Some("regions"));
        assert_eq!(
            target_search_collection_family("surfaces"),
            Some("surfaces")
        );
        assert_eq!(target_search_collection_family("all"), Some("all"));
        assert!(target_search_candidate_matches(
            "controls",
            HintKind::Button
        ));
        assert!(target_search_candidate_matches(
            "controls",
            HintKind::EditorFocus
        ));
        assert!(!target_search_candidate_matches(
            "controls",
            HintKind::Image
        ));
        assert!(!target_search_candidate_matches("controls", HintKind::Link));
        assert!(target_search_candidate_matches("all", HintKind::Surface));
    }
}
