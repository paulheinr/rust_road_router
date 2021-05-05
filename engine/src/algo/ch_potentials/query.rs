use super::*;

use crate::datastr::rank_select_map::FastClearBitVec;
use crate::{
    algo::{a_star::ZeroPotential, dijkstra::gen_topo_dijkstra::*, topocore::*},
    datastr::graph::time_dependent::*,
};

pub struct Server<Graph = OwnedGraph, Ops = DefaultOps, P = ZeroPotential>
where
    Ops: DijkstraOps<Graph>,
{
    forward_dijkstra: GenTopoDijkstra<VirtualTopocoreGraph<Graph>, VirtualTopocoreOps<Ops>>,
    #[cfg(not(feature = "chpot-no-bcc"))]
    into_comp_graph: VirtualTopocoreGraph<Graph>,
    #[cfg(not(feature = "chpot-no-bcc"))]
    reversed_into_comp_graph: UnweightedOwnedGraph,

    potential: P,

    reversed: UnweightedOwnedGraph,
    virtual_topocore: VirtualTopocore,
    visited: FastClearBitVec,
}

impl<Graph, Ops: DijkstraOps<Graph, Label = Timestamp>, P: Potential> Server<Graph, Ops, P>
where
    Graph: for<'a> LinkIterable<'a, NodeId> + for<'a> LinkIterable<'a, Ops::Arc>,
{
    pub fn new<G>(graph: &G, potential: P, ops: Ops) -> Self
    where
        G: for<'a> LinkIterable<'a, NodeId>,
        Graph: BuildPermutated<G>,
    {
        report_time_with_key("TopoDijkstra preprocessing", "topo_dijk_prepro", move || {
            let n = graph.num_nodes();
            #[cfg(feature = "chpot-no-bcc")]
            {
                let (graph, virtual_topocore) = VirtualTopocoreGraph::new(graph);
                let reversed = UnweightedOwnedGraph::reversed(&graph);
                Self {
                    forward_dijkstra: GenTopoDijkstra::new_with_ops(graph, VirtualTopocoreOps(ops)),
                    potential,

                    reversed,
                    virtual_topocore,
                    visited: FastClearBitVec::new(n),
                }
            }
            #[cfg(not(feature = "chpot-no-bcc"))]
            {
                let (main_graph, into_comp_graph, virtual_topocore) = VirtualTopocoreGraph::new_topo_dijkstra_graphs(graph);
                let reversed = UnweightedOwnedGraph::reversed(&main_graph);
                let reversed_into_comp_graph = UnweightedOwnedGraph::reversed(&into_comp_graph);

                Self {
                    forward_dijkstra: GenTopoDijkstra::new_with_ops(main_graph, VirtualTopocoreOps(ops)),
                    into_comp_graph,
                    reversed_into_comp_graph,
                    potential,

                    reversed,
                    virtual_topocore,
                    visited: FastClearBitVec::new(n),
                }
            }
        })
    }

    fn dfs(
        graph: &UnweightedOwnedGraph,
        node: NodeId,
        visited: &mut FastClearBitVec,
        border_callback: &mut impl FnMut(NodeId),
        in_core: &mut impl FnMut(NodeId) -> bool,
    ) {
        if visited.get(node as usize) {
            return;
        }
        visited.set(node as usize);
        if in_core(node) {
            border_callback(node);
            return;
        }
        for head in graph.link_iter(node) {
            Self::dfs(graph, head, visited, border_callback, in_core);
        }
    }

    #[cfg(not(feature = "chpot-no-bcc"))]
    fn border(&mut self, node: NodeId) -> Option<NodeId> {
        let mut border = None;
        self.visited.clear();
        let virtual_topocore = &self.virtual_topocore;
        Self::dfs(
            &self.reversed_into_comp_graph,
            node,
            &mut self.visited,
            &mut |node| {
                let prev = border.replace(node);
                debug_assert_eq!(prev, None);
            },
            &mut |node| virtual_topocore.node_type(node).in_core(),
        );
        border
    }

    fn distance(
        &mut self,
        mut query: impl GenQuery<Timestamp> + Copy,
        mut inspect: impl FnMut(NodeId, &NodeOrder, &GenTopoDijkstra<VirtualTopocoreGraph<Graph>, VirtualTopocoreOps<Ops>>, &mut P),
    ) -> Option<Weight> {
        let to = query.to();
        query.permutate(&self.virtual_topocore.order);

        report!("algo", "CH Potentials Query");

        let departure = query.initial_state();

        let mut num_queue_pops = 0;

        self.forward_dijkstra.initialize_query(query);
        self.potential.init(to);
        #[cfg(not(feature = "chpot-no-bcc"))]
        let border = self.border(query.to());
        let forward_dijkstra = &mut self.forward_dijkstra;
        let virtual_topocore = &self.virtual_topocore;
        let potential = &mut self.potential;

        if cfg!(feature = "chpot-no-bcc") || self.virtual_topocore.node_type(query.to()).in_core() {
            let mut counter = 0;
            self.visited.clear();
            Self::dfs(&self.reversed, query.to(), &mut self.visited, &mut |_| {}, &mut |_| {
                if counter < 100 {
                    counter += 1;
                    false
                } else {
                    true
                }
            });

            if counter < 100 {
                return None;
            }
        }

        #[cfg(not(feature = "chpot-no-bcc"))]
        {
            let border_node = if let Some(border_node) = border { border_node } else { return None };
            let border_node_pot = if let Some(pot) = potential.potential(self.virtual_topocore.order.node(border_node)) {
                pot
            } else {
                return None;
            };

            while let Some(node) = forward_dijkstra.next_step_with_potential(|node| potential.potential(virtual_topocore.order.node(node))) {
                num_queue_pops += 1;
                inspect(node, &virtual_topocore.order, forward_dijkstra, potential);

                if node == query.to()
                    || forward_dijkstra
                        .queue()
                        .peek()
                        .map(|e| e.key >= *forward_dijkstra.tentative_distance(border_node) + border_node_pot)
                        .unwrap_or(false)
                {
                    break;
                }
            }

            forward_dijkstra.swap_graph(&mut self.into_comp_graph);
            forward_dijkstra.reinit_queue(border_node);
        }

        while let Some(node) = forward_dijkstra.next_step_with_potential(|node| potential.potential(virtual_topocore.order.node(node))) {
            num_queue_pops += 1;
            inspect(node, &virtual_topocore.order, forward_dijkstra, potential);

            if node == query.to()
                || forward_dijkstra
                    .queue()
                    .peek()
                    .map(|e| e.key >= *forward_dijkstra.tentative_distance(query.to()))
                    .unwrap_or(false)
            {
                break;
            }
        }

        #[cfg(not(feature = "chpot-no-bcc"))]
        forward_dijkstra.swap_graph(&mut self.into_comp_graph);

        report!("num_queue_pops", num_queue_pops);
        report!("num_queue_pushs", forward_dijkstra.num_queue_pushs());
        report!("num_relaxed_arcs", forward_dijkstra.num_relaxed_arcs());

        let dist = *forward_dijkstra.tentative_distance(query.to());
        if dist < INFINITY {
            Some(dist - departure)
        } else {
            None
        }
    }

    pub fn visualize_query(&mut self, query: impl GenQuery<Timestamp> + Copy, lat: &[f32], lng: &[f32]) -> Option<Weight> {
        let mut num_settled_nodes = 0;
        let res = self.distance(query, |node, order, dijk, pot| {
            let node_id = order.node(node) as usize;
            println!(
                "var marker = L.marker([{}, {}], {{ icon: L.dataIcon({{ data: {{ popped: {} }}, ...blueIconOptions }}) }}).addTo(map);",
                lat[node_id], lng[node_id], num_settled_nodes
            );
            println!(
                "marker.bindPopup(\"id: {}<br>distance: {}<br>potential: {}\");",
                node_id,
                dijk.tentative_distance(node),
                pot.potential(node_id as NodeId).unwrap()
            );
            num_settled_nodes += 1;
        });
        println!(
            "L.marker([{}, {}], {{ title: \"from\", icon: blackIcon }}).addTo(map);",
            lat[query.from() as usize],
            lng[query.from() as usize]
        );
        println!(
            "L.marker([{}, {}], {{ title: \"from\", icon: blackIcon }}).addTo(map);",
            lat[query.to() as usize],
            lng[query.to() as usize]
        );
        res
    }

    fn path(&self, mut query: impl GenQuery<Timestamp>) -> Vec<NodeId> {
        query.permutate(&self.virtual_topocore.order);
        let mut path = Vec::new();
        path.push(query.to());

        while *path.last().unwrap() != query.from() {
            let next = self.forward_dijkstra.predecessor(*path.last().unwrap());
            path.push(next);
        }

        path.reverse();
        // permute path back??

        path
    }
}

pub struct PathServerWrapper<'s, G, O: DijkstraOps<G>, P, Q>(&'s mut Server<G, O, P>, Q);

impl<'s, G, O, P, Q> PathServer for PathServerWrapper<'s, G, O, P, Q>
where
    P: Potential,
    O: DijkstraOps<G, Label = Timestamp>,
    G: for<'a> LinkIterable<'a, NodeId> + for<'a> LinkIterable<'a, O::Arc>,
    Q: GenQuery<Timestamp> + Copy,
{
    type NodeInfo = NodeId;

    fn path(&mut self) -> Vec<Self::NodeInfo> {
        Server::path(self.0, self.1)
    }
}

impl<'s, G, O, P, Q> PathServerWrapper<'s, G, O, P, Q>
where
    P: Potential,
    O: DijkstraOps<G, Label = Timestamp>,
    G: for<'a> LinkIterable<'a, NodeId> + for<'a> LinkIterable<'a, O::Arc>,
    Q: GenQuery<Timestamp> + Copy,
{
    /// Print path with debug info as js to stdout.
    pub fn debug_path(&mut self, lat: &[f32], lng: &[f32]) {
        for node in self.path() {
            println!(
                "var marker = L.marker([{}, {}], {{ icon: blackIcon }}).addTo(map);",
                lat[node as usize], lng[node as usize]
            );
            let dist = *self.0.forward_dijkstra.tentative_distance(node);
            let pot = self.lower_bound(node).unwrap_or(INFINITY);
            println!(
                "marker.bindPopup(\"id: {}<br>distance: {}<br>lower_bound: {}<br>sum: {}\");",
                node,
                dist / 1000,
                pot / 1000,
                (pot + dist) / 1000
            );
        }
    }

    pub fn potential(&self) -> &P {
        &self.0.potential
    }

    pub fn lower_bound(&mut self, node: NodeId) -> Option<Weight> {
        self.0.potential.potential(node)
    }
}

impl<'s, G: 's, O: 's, P: 's> TDQueryServer<'s, Timestamp, Weight> for Server<G, O, P>
where
    P: Potential,
    O: DijkstraOps<G, Label = Timestamp>,
    G: for<'a> LinkIterable<'a, NodeId> + for<'a> LinkIterable<'a, O::Arc>,
{
    type P = PathServerWrapper<'s, G, O, P, TDQuery<Timestamp>>;

    fn query(&'s mut self, query: TDQuery<Timestamp>) -> Option<QueryResult<Self::P, Weight>> {
        self.distance(query, |_, _, _, _| ())
            .map(move |distance| QueryResult::new(distance, PathServerWrapper(self, query)))
    }
}

impl<'s, G: 's, O: 's, P: 's> QueryServer<'s> for Server<G, O, P>
where
    P: Potential,
    O: DijkstraOps<G, Label = Timestamp>,
    G: for<'a> LinkIterable<'a, NodeId> + for<'a> LinkIterable<'a, O::Arc>,
{
    type P = PathServerWrapper<'s, G, O, P, Query>;

    fn query(&'s mut self, query: Query) -> Option<QueryResult<Self::P, Weight>> {
        self.distance(query, |_, _, _, _| ())
            .map(move |distance| QueryResult::new(distance, PathServerWrapper(self, query)))
    }
}

struct VirtualTopocoreOps<O>(O);

impl<G, O> DijkstraOps<VirtualTopocoreGraph<G>> for VirtualTopocoreOps<O>
where
    O: DijkstraOps<G>,
{
    type Label = O::Label;
    type Arc = O::Arc;
    type LinkResult = O::LinkResult;

    #[inline(always)]
    fn link(&mut self, graph: &VirtualTopocoreGraph<G>, label: &Self::Label, link: &Self::Arc) -> Self::LinkResult {
        self.0.link(&graph.graph, label, link)
    }

    #[inline(always)]
    fn merge(&mut self, label: &mut Self::Label, linked: Self::LinkResult) -> bool {
        self.0.merge(label, linked)
    }
}
