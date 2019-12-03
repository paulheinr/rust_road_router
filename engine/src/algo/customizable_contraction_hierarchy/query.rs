use super::*;
pub mod stepped_elimination_tree;
use stepped_elimination_tree::SteppedEliminationTree;

#[derive(Debug)]
pub struct Server<'a> {
    forward: SteppedEliminationTree<'a, FirstOutGraph<&'a [EdgeId], &'a [NodeId], Vec<Weight>>>,
    backward: SteppedEliminationTree<'a, FirstOutGraph<&'a [EdgeId], &'a [NodeId], Vec<Weight>>>,
    cch_graph: &'a CCH,
    tentative_distance: Weight,
    meeting_node: NodeId,
}

impl<'a> Server<'a> {
    pub fn new(customized: Customized<'a>) -> Server<'a> {
        let cch = customized.cch;
        let (forward, backward) = customized.into_ch_graphs();
        let forward = SteppedEliminationTree::new(forward, cch.elimination_tree());
        let backward = SteppedEliminationTree::new(backward, cch.elimination_tree());

        Server {
            forward,
            backward,
            cch_graph: cch,
            tentative_distance: INFINITY,
            meeting_node: 0,
        }
    }

    pub fn update(&mut self, mut customized: Customized<'a>) {
        self.forward.graph_mut().swap_weights(&mut customized.upward);
        self.backward.graph_mut().swap_weights(&mut customized.downward);
    }

    fn distance(&mut self, from: NodeId, to: NodeId) -> Option<Weight> {
        let from = self.cch_graph.node_order().rank(from);
        let to = self.cch_graph.node_order().rank(to);

        // initialize
        self.tentative_distance = INFINITY;
        self.meeting_node = 0;
        self.forward.initialize_query(from);
        self.backward.initialize_query(to);

        while self.forward.next().is_some() {
            self.forward.next_step();
        }

        while let QueryProgress::Progress(State { distance, node }) = self.backward.next_step() {
            if distance + self.forward.tentative_distance(node) < self.tentative_distance {
                self.tentative_distance = distance + self.forward.tentative_distance(node);
                self.meeting_node = node;
            }
        }

        match self.tentative_distance {
            INFINITY => None,
            dist => Some(dist),
        }
    }

    fn path(&mut self) -> Vec<NodeId> {
        self.forward
            .unpack_path(self.meeting_node, true, self.cch_graph, self.backward.graph().weight());
        self.backward
            .unpack_path(self.meeting_node, true, self.cch_graph, self.forward.graph().weight());

        let mut path = Vec::new();
        path.push(self.meeting_node);

        while *path.last().unwrap() != self.forward.origin() {
            path.push(self.forward.predecessor(*path.last().unwrap()));
        }

        path.reverse();

        while *path.last().unwrap() != self.backward.origin() {
            path.push(self.backward.predecessor(*path.last().unwrap()));
        }

        for node in &mut path {
            *node = self.cch_graph.node_order().node(*node);
        }

        path
    }
}

pub struct PathServerWrapper<'s, 'a>(&'s mut Server<'a>);

impl<'s, 'a> PathServer<'s> for PathServerWrapper<'s, 'a> {
    type NodeInfo = NodeId;

    fn path(&mut self) -> Vec<Self::NodeInfo> {
        Server::path(self.0)
    }
}

impl<'s, 'a: 's> QueryServer<'s> for Server<'a> {
    type P = PathServerWrapper<'s, 'a>;

    fn query(&'s mut self, query: Query) -> Option<QueryResult<Self::P, Weight>> {
        self.distance(query.from, query.to).map(move |distance| QueryResult {
            distance,
            path_server: PathServerWrapper(self),
        })
    }
}