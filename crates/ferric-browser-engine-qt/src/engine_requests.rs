//! Projection of captured reducer effects into native request records.

use cxx_qt_lib::{QMap, QMapPair_QString_QVariant, QString, QVariant};
use ferric_browser_core::{EngineEffect, SearchCase, Target};

#[derive(Clone, Debug)]
pub(super) enum NativeRequest {
    Engine(EngineEffect),
    View(ferric_browser_application::ViewRequest),
    Presentation(String),
}

pub(super) fn target(effect: &EngineEffect) -> Option<Target> {
    match effect {
        EngineEffect::Navigate { target, .. }
        | EngineEffect::Reload { target, .. }
        | EngineEffect::ClipboardEdit { target, .. }
        | EngineEffect::Stop { target }
        | EngineEffect::TraverseHistory { target, .. }
        | EngineEffect::FindText { target, .. }
        | EngineEffect::ClearFindText { target } => Some(*target),
        _ => None,
    }
}

pub(super) fn project(effect: &EngineEffect) -> QVariant {
    let mut row = QMap::<QMapPair_QString_QVariant>::default();
    let mut text = |key: &str, value: &str| {
        row.insert(QString::from(key), QVariant::from(&QString::from(value)));
    };
    if let Some(target) = target(effect) {
        text("tabId", &target.tab.to_string());
        text("generation", &target.generation.to_string());
        text("documentId", &target.document.to_string());
    }
    match effect {
        EngineEffect::Navigate { url, .. } => {
            text("kind", "navigate");
            text("url", url.as_str());
        }
        EngineEffect::Reload { bypass_cache, .. } => {
            text("kind", "reload");
            row.insert(QString::from("bypassCache"), QVariant::from(bypass_cache));
        }
        EngineEffect::Stop { .. } => text("kind", "stop"),
        EngineEffect::TraverseHistory { offset, .. } => {
            text("kind", "history");
            row.insert(QString::from("offset"), QVariant::from(offset));
        }
        EngineEffect::FindText {
            query,
            backward,
            case,
            ..
        } => {
            text("kind", "find");
            text("query", query);
            text(
                "caseMode",
                match case {
                    SearchCase::Smart => "smart",
                    SearchCase::Sensitive => "sensitive",
                    SearchCase::Insensitive => "insensitive",
                },
            );
            row.insert(QString::from("backward"), QVariant::from(backward));
        }
        EngineEffect::ClearFindText { .. } => text("kind", "clear-find"),
        EngineEffect::ClipboardEdit { operation, .. } => {
            text("kind", "edit");
            text("operation", operation.as_str());
        }
        _ => return QVariant::default(),
    }
    QVariant::from(&row)
}

impl NativeRequest {
    pub(super) fn captured_target(&self) -> Option<Target> {
        match self {
            Self::Engine(effect) => target(effect),
            Self::View(request) => Some(request.target),
            Self::Presentation(_) => None,
        }
    }
    pub(super) fn projection(&self) -> QVariant {
        match self {
            Self::Engine(effect) => project(effect),
            Self::View(request) => project_view(request),
            Self::Presentation(_) => QVariant::default(),
        }
    }
}

fn project_view(request: &ferric_browser_application::ViewRequest) -> QVariant {
    use ferric_browser_application::{
        ScriptWorld, ScrollEdge, ScrollTargetPolicy, VerticalDirection, ViewCommand,
    };
    fn text(row: &mut QMap<QMapPair_QString_QVariant>, key: &str, value: &str) {
        row.insert(QString::from(key), QVariant::from(&QString::from(value)));
    }
    let mut row = QMap::<QMapPair_QString_QVariant>::default();
    text(&mut row, "tabId", &request.target.tab.to_string());
    text(
        &mut row,
        "generation",
        &request.target.generation.to_string(),
    );
    text(&mut row, "documentId", &request.target.document.to_string());
    let kind = match &request.command {
        ViewCommand::Scroll { direction, count } => {
            text(&mut row, "direction", direction.as_str());
            row.insert(QString::from("count"), QVariant::from(count));
            "scroll"
        }
        ViewCommand::ScrollPage {
            direction,
            half,
            count,
        } => {
            text(
                &mut row,
                "direction",
                match direction {
                    VerticalDirection::Up => "up",
                    VerticalDirection::Down => "down",
                },
            );
            row.insert(QString::from("count"), QVariant::from(count));
            row.insert(QString::from("half"), QVariant::from(half));
            "scroll-page"
        }
        ViewCommand::ScrollTo { edge } => {
            text(
                &mut row,
                "direction",
                match edge {
                    ScrollEdge::Top => "top",
                    ScrollEdge::Bottom => "bottom",
                },
            );
            "scroll-to"
        }
        ViewCommand::ScrollTarget(policy) => {
            text(
                &mut row,
                "action",
                match policy {
                    ScrollTargetPolicy::Auto => "auto",
                    ScrollTargetPolicy::Document => "document",
                    ScrollTargetPolicy::Status => "status",
                },
            );
            "scroll-target"
        }
        ViewCommand::Zoom { hundredths } => {
            row.insert(
                QString::from("factor"),
                QVariant::from(&(f64::from(*hundredths) / 100.0)),
            );
            "zoom"
        }
        ViewCommand::PrintPdf { path } => {
            text(&mut row, "path", &path.to_string_lossy());
            "print-pdf"
        }
        ViewCommand::SavePage { path } => {
            text(&mut row, "path", &path.to_string_lossy());
            "save-page"
        }
        ViewCommand::ViewSource { url } => {
            text(&mut row, "url", url.as_str());
            "view-source"
        }
        ViewCommand::DeveloperTools { detach } => {
            row.insert(QString::from("detach"), QVariant::from(detach));
            "devtools"
        }
        ViewCommand::Print => "print",
        ViewCommand::SiteDoctorReload { experiment_id } => {
            text(&mut row, "experimentId", experiment_id);
            "site-doctor-reload"
        }
        ViewCommand::SiteDoctorFreshView { experiment_id, url } => {
            text(&mut row, "experimentId", experiment_id);
            text(&mut row, "url", url.as_str());
            "site-doctor-fresh-view"
        }
        ViewCommand::Evaluate { script, world } => {
            text(&mut row, "script", script);
            text(
                &mut row,
                "world",
                match world {
                    ScriptWorld::Application => "application",
                    ScriptWorld::Page => "page",
                },
            );
            "evaluate"
        }
    };
    text(&mut row, "kind", kind);
    QVariant::from(&row)
}
