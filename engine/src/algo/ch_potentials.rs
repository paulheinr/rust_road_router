use super::*;
use crate::{
    algo::customizable_contraction_hierarchy::{query::stepped_elimination_tree::SteppedEliminationTree, *},
    datastr::timestamped_vector::TimestampedVector,
    util::in_range_option::InRangeOption,
};

pub mod query;
pub mod td_query;

pub trait Potential {
    fn init(&mut self, target: NodeId);
    fn potential(&mut self, node: NodeId) -> Option<Weight>;
    fn num_pot_evals(&self) -> usize;
}

#[derive(Debug)]
pub struct CCHPotential<'a> {
    cch: &'a CCH,
    stack: Vec<NodeId>,
    potentials: TimestampedVector<InRangeOption<Weight>>,
    forward_cch_graph: FirstOutGraph<&'a [EdgeId], &'a [NodeId], Vec<Weight>>,
    backward_elimination_tree: SteppedEliminationTree<'a, FirstOutGraph<&'a [EdgeId], &'a [NodeId], Vec<Weight>>>,
    num_pot_evals: usize,
}

impl<'a> CCHPotential<'a> {
    pub fn new<Graph>(cch: &'a CCH, lower_bound: &Graph) -> Self
    where
        Graph: for<'b> LinkIterGraph<'b> + RandomLinkAccessGraph + Sync,
    {
        let customized = customize(cch, lower_bound);
        let (forward_up_graph, backward_up_graph) = customized.into_ch_graphs();
        let backward_elimination_tree = SteppedEliminationTree::new(backward_up_graph, cch.elimination_tree());

        Self {
            cch,
            stack: Vec::new(),
            forward_cch_graph: forward_up_graph,
            backward_elimination_tree,
            potentials: TimestampedVector::new(cch.num_nodes(), InRangeOption::new(None)),
            num_pot_evals: 0,
        }
    }
}

impl<'a> Potential for CCHPotential<'a> {
    fn init(&mut self, target: NodeId) {
        self.potentials.reset();
        self.backward_elimination_tree.initialize_query(self.cch.node_order().rank(target));
        while self.backward_elimination_tree.next().is_some() {
            self.backward_elimination_tree.next_step();
        }
        self.num_pot_evals = 0;
    }

    fn potential(&mut self, node: NodeId) -> Option<u32> {
        let node = self.cch.node_order().rank(node);
        if self.potentials[node as usize].value().is_none() {
            self.num_pot_evals += 1;
        }
        let mut cur_node = node;
        while self.potentials[cur_node as usize].value().is_none() {
            self.stack.push(cur_node);
            if let Some(parent) = self.backward_elimination_tree.parent(cur_node).value() {
                cur_node = parent;
            } else {
                break;
            }
        }

        while let Some(node) = self.stack.pop() {
            let min_by_up = self
                .forward_cch_graph
                .neighbor_iter(node)
                .map(|edge| edge.weight + self.potentials[edge.node as usize].value().unwrap())
                .min()
                .unwrap_or(INFINITY);

            self.potentials[node as usize] = InRangeOption::new(Some(std::cmp::min(self.backward_elimination_tree.tentative_distance(node), min_by_up)));
        }

        let dist = self.potentials[node as usize].value().unwrap();
        if dist < INFINITY {
            Some(dist)
        } else {
            None
        }
    }

    fn num_pot_evals(&self) -> usize {
        self.num_pot_evals
    }
}
