//! Typed journey-graph projection for QML library components.

use super::{Pin, QString, QStringList, qobject};

/// One directed relationship shown in the bounded journey graph.
pub(super) struct JourneyGraphEdgePresentation {
    pub(super) source: String,
    pub(super) target: String,
    pub(super) transition: String,
}

impl qobject::BrowserUi {
    pub(super) fn set_library_graph_edges(
        mut self: Pin<&mut Self>,
        edges: &[JourneyGraphEdgePresentation],
    ) {
        self.as_mut().set_library_graph_edge_sources(
            edges
                .iter()
                .map(|edge| QString::from(&edge.source))
                .collect(),
        );
        self.as_mut().set_library_graph_edge_targets(
            edges
                .iter()
                .map(|edge| QString::from(&edge.target))
                .collect(),
        );
        self.as_mut().set_library_graph_edge_transitions(
            edges
                .iter()
                .map(|edge| QString::from(&edge.transition))
                .collect(),
        );
    }

    pub(super) fn clear_library_graph_edges(mut self: Pin<&mut Self>) {
        self.as_mut()
            .set_library_graph_edge_sources(QStringList::default());
        self.as_mut()
            .set_library_graph_edge_targets(QStringList::default());
        self.as_mut()
            .set_library_graph_edge_transitions(QStringList::default());
    }
}
