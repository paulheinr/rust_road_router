use super::*;
use self::timestamped_vector::TimestampedVector;
use crate::index_heap::{IndexdMinHeap, Indexing};
use crate::graph::floating_time_dependent::*;

#[derive(Debug, Clone)]
pub enum QueryProgress {
    Progress(State),
    Done(Option<FlWeight>),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct State {
    pub distance: Timestamp,
    pub node: NodeId,
}

impl Indexing for State {
    fn as_index(&self) -> usize {
        self.node as usize
    }
}

#[derive(Debug)]
pub struct FloatingTDSteppedDijkstra {
    graph: TDGraph,
    distances: TimestampedVector<Timestamp>,
    predecessors: Vec<NodeId>,
    closest_node_priority_queue: IndexdMinHeap<State>,
    // the current query
    query: Option<FlTDQuery>,
    // first option: algorithm finished? second option: final result of algorithm
    result: Option<Option<FlWeight>>,
}

impl FloatingTDSteppedDijkstra {
    pub fn new(graph: TDGraph) -> FloatingTDSteppedDijkstra {
        let n = graph.num_nodes();

        FloatingTDSteppedDijkstra {
            graph,
            // initialize tentative distances to INFINITY
            distances: TimestampedVector::new(n, Timestamp::new(f64::from(INFINITY))),
            predecessors: vec![n as NodeId; n],
            closest_node_priority_queue: IndexdMinHeap::new(n),
            query: None,
            result: None,
        }
    }

    pub fn initialize_query(&mut self, query: FlTDQuery) {
        let from = query.from;
        // initialize
        self.query = Some(query);
        self.result = None;
        self.closest_node_priority_queue.clear();
        self.distances.reset();

        // Starte with origin
        self.distances.set(from as usize, query.departure_time);
        self.closest_node_priority_queue.push(State { distance: query.departure_time, node: from });
    }

    pub fn next_step<F: Fn(EdgeId) -> bool>(&mut self, check_edge: F) -> QueryProgress {
        match self.result {
            Some(result) => QueryProgress::Done(result),
            None => {
                self.settle_next_node(check_edge)
            }
        }
    }

    fn settle_next_node<F: Fn(EdgeId) -> bool>(&mut self, check_edge: F) -> QueryProgress {
        let to = self.query().to;

        // Examine the frontier with lower distance nodes first (min-heap)
        if let Some(State { distance, node }) = self.closest_node_priority_queue.pop() {
            // Alternatively we could have continued to find all shortest paths
            if node == to {
                let result = Some(distance - self.query().departure_time);
                self.result = Some(result);
                return QueryProgress::Done(result);
            }

            // For each node we can reach, see if we can find a way with
            // a lower distance going through this node
            for (&neighbor, edge_id) in self.graph.neighbor_and_edge_id_iter(node) {
                if check_edge(edge_id) {
                    let plf = self.graph.travel_time_function(edge_id);
                    let next = State { distance: distance + plf.evaluate(distance), node: neighbor };

                    if next.distance < self.distances[next.node as usize] {
                        self.distances.set(next.node as usize, next.distance);
                        self.predecessors[next.node as usize] = node;
                        if self.closest_node_priority_queue.contains_index(next.as_index()) {
                            self.closest_node_priority_queue.decrease_key(next);
                        } else {
                            self.closest_node_priority_queue.push(next);
                        }
                    }
                }
            }

            QueryProgress::Progress(State { distance, node })
        } else {
            self.result = Some(None);
            QueryProgress::Done(None)
        }
    }

    pub fn tentative_distance(&self, node: NodeId) -> Timestamp {
        self.distances[node as usize]
    }

    pub fn predecessor(&self, node: NodeId) -> NodeId {
        self.predecessors[node as usize]
    }

    pub fn query(&self) -> &FlTDQuery {
        self.query.as_ref().expect("query was not initialized properly")
    }

    pub fn graph(&self) -> &TDGraph {
        &self.graph
    }
}
