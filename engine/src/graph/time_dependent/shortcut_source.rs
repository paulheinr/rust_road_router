use super::*;
use in_range_option::InRangeOption;

#[derive(Debug, Clone, Copy)]
pub enum ShortcutSource {
    Shortcut(EdgeId, EdgeId),
    OriginalEdge(EdgeId),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShortcutData {
    down_arc: InRangeOption<EdgeId>,
    up_arc: EdgeId
}

impl ShortcutData {
    pub fn new(source: ShortcutSource) -> ShortcutData {
        match source {
            ShortcutSource::Shortcut(down, up) => ShortcutData { down_arc: InRangeOption::new(Some(down)), up_arc: up },
            ShortcutSource::OriginalEdge(edge) => ShortcutData { down_arc: InRangeOption::new(None), up_arc: edge },
        }
    }

    pub fn source(self) -> ShortcutSource {
        match self.down_arc.value() {
            Some(down_shortcut_id) => ShortcutSource::Shortcut(down_shortcut_id, self.up_arc),
            None => ShortcutSource::OriginalEdge(self.up_arc),
        }
    }

    pub fn evaluate(self, departure: Timestamp, shortcut_graph: &ShortcutGraph) -> Weight {
        debug_assert!(departure < period());
        match self.down_arc.value() {
            Some(down_shortcut_id) => {
                Linked::new(down_shortcut_id, self.up_arc).evaluate(departure, shortcut_graph)
            },
            None => {
                shortcut_graph.original_graph().travel_time_function(self.up_arc).evaluate(departure)
            },
        }
    }

    pub fn bounds(self, shortcut_graph: &ShortcutGraph) -> (Weight, Weight) {
        match self.down_arc.value() {
            Some(down_shortcut_id) => {
                Linked::new(down_shortcut_id, self.up_arc).bounds(shortcut_graph)
            },
            None => {
                shortcut_graph.original_graph().travel_time_function(self.up_arc).bounds()
            },
        }
    }

    pub fn debug_to_s(self, shortcut_graph: &ShortcutGraph, indent: usize) -> String {
        println!("{:?}", self.source());
        match self.down_arc.value() {
            Some(down_shortcut_id) => {
                Linked::new(down_shortcut_id, self.up_arc).debug_to_s(shortcut_graph, indent)
            },
            None => {
                shortcut_graph.original_graph().travel_time_function(self.up_arc).debug_to_s(indent)
            },
        }
    }

    pub fn validate_does_not_contain(&self, edge_id: EdgeId, shortcut_graph: &ShortcutGraph) {
        if let Some(down_shortcut_id) = self.down_arc.value() {
            assert_ne!(edge_id, down_shortcut_id);
            assert_ne!(edge_id, self.up_arc);
            Linked::new(down_shortcut_id, self.up_arc).validate_does_not_contain(edge_id, shortcut_graph);
        }
    }
}

pub(super) enum ShortcutSourceSegmentIter<'a, PLFIter: Iterator<Item = PLFSeg>> {
    Shortcut(Box<dyn Iterator<Item = MATSeg> + 'a>),
    OriginalEdge(PLFIter),
}

impl<'a, PLFIter: Iterator<Item = PLFSeg>> Iterator for ShortcutSourceSegmentIter<'a, PLFIter> {
    type Item = MATSeg;

    fn next(&mut self) -> Option<Self::Item> {
        // println!("shortcut src next");
        match *self {
            ShortcutSourceSegmentIter::Shortcut(ref mut iter) => iter.next(),
            ShortcutSourceSegmentIter::OriginalEdge(ref mut iter) => iter.next().map(|seg| seg.into_atfseg()),
        }
    }
}
