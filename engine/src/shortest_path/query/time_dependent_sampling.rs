use super::*;
use shortest_path::customizable_contraction_hierarchy::cch_graph::CCHGraph;
use shortest_path::td_stepped_dijkstra::TDSteppedDijkstra;
use graph::time_dependent::*;
use graph::RandomLinkAccessGraph;
use shortest_path::query::customizable_contraction_hierarchy::Server as CCHServer;
use rank_select_map::BitVec;
use super::td_stepped_dijkstra::QueryProgress;

use std::collections::LinkedList;
use std::ops::Range;

#[derive(Debug)]
pub struct Server<'a> {
    dijkstra: TDSteppedDijkstra,
    samples: Vec<CCHServer<'a>>
}

impl<'a> Server<'a> {
    pub fn new(graph: TDGraph, cch: &'a CCHGraph) -> Server<'a> {
        let hour = period() / 24;
        let samples = vec![
            Range { start: 22, end: 5 },
            Range { start: 7, end: 10 },
            Range { start: 11, end: 15 },
            Range { start: 16, end: 19 },
        ].into_iter().map(|range| {
            let range = Range { start: range.start * hour, end: range.end * hour };
            WrappingRange::new(range)
        }).map(|range| {
            (0..graph.num_arcs() as EdgeId)
                .map(|edge_id| graph.travel_time_function(edge_id).average(range.clone()))
                .collect::<Vec<Weight>>()
        }).map(|metric| {
            CCHServer::new(cch, &FirstOutGraph::new(graph.first_out(), graph.head(), metric))
        }).collect();

        Server {
            dijkstra: TDSteppedDijkstra::new(graph),
            samples
        }
    }

    pub fn distance(&mut self, from: NodeId, to: NodeId, departure_time: Timestamp) -> Option<Weight> {
        let mut active_edges = BitVec::new(self.dijkstra.graph().num_arcs());

        for server in &mut self.samples {
            server.distance(from, to);
            let path = server.path();
            let path_iter = path.iter();
            let mut second_node_iter = path_iter.clone();
            second_node_iter.next();

            for (first_node, second_node) in path_iter.zip(second_node_iter) {
                active_edges.set(self.dijkstra.graph().edge_index(*first_node, *second_node).unwrap() as usize);
            }
        }

        self.dijkstra.initialize_query(TDQuery { from, to, departure_time }, active_edges);

        loop {
            match self.dijkstra.next_step() {
                QueryProgress::Progress(_) => continue,
                QueryProgress::Done(result) => return result
            }
        }
    }

    pub fn is_in_searchspace(&self, node: NodeId) -> bool {
        self.dijkstra.tentative_distance(node) < INFINITY
    }

    pub fn path(&self) -> LinkedList<NodeId> {
        let mut path = LinkedList::new();
        path.push_front(self.dijkstra.query().to);

        while *path.front().unwrap() != self.dijkstra.query().from {
            let next = self.dijkstra.predecessor(*path.front().unwrap());
            path.push_front(next);
        }

        path
    }
}
