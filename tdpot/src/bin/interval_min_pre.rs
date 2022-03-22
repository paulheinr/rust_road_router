// WIP: CH potentials for TD Routing.

use rust_road_router::{algo::customizable_contraction_hierarchy::*, cli::CliErr, datastr::graph::*, datastr::node_order::*, io::*};
use std::{env, error::Error, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let arg = &args.next().ok_or(CliErr("No directory arg given"))?;
    let path = Path::new(arg);

    let graph = floating_time_dependent::TDGraph::reconstruct_from(&path)?;
    let order = NodeOrder::from_node_order(Vec::load_from(path.join("cch_perm"))?);
    let cch = CCH::fix_order_and_build(&graph, order);

    let catchup = customization::ftd_for_pot::customize::<96>(&cch, &graph);

    let customized_folder = path.join(args.next().unwrap_or("customized_corridor_mins".to_string()));
    catchup.deconstruct_to(&customized_folder)?;

    Ok(())
}