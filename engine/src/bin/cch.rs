// Example of complete CCH toolchain.
// Takes a directory as argument, which has to contain the graph (in RoutingKit format), a nested disection order and queries.

use std::{env, error::Error, path::Path};

use rust_road_router::{
    algo::customizable_contraction_hierarchy::{query::Server, *},
    cli::CliErr,
    datastr::{graph::*, node_order::NodeOrder},
    experiments,
    io::Load,
};

fn main() -> Result<(), Box<dyn Error>> {
    let arg = &env::args().skip(1).next().ok_or(CliErr("No directory arg given"))?;
    let path = Path::new(arg);

    let first_out = Vec::load_from(path.join("first_out"))?;
    let head = Vec::load_from(path.join("head"))?;
    let travel_time = Vec::load_from(path.join("travel_time"))?;

    let graph = FirstOutGraph::new(&first_out[..], &head[..], &travel_time[..]);
    let cch_order = Vec::load_from(path.join("cch_perm"))?;
    let cch_order = NodeOrder::from_node_order(cch_order);

    let cch = contract(&graph, cch_order);
    let cch_order = CCHReordering {
        cch: &cch,
        latitude: &[],
        longitude: &[],
    }
    .reorder_for_seperator_based_customization();
    let cch = contract(&graph, cch_order);

    let mut server = Server::new(customize(&cch, &graph));

    let from = Vec::load_from(path.join("test/source"))?;
    let to = Vec::load_from(path.join("test/target"))?;
    let ground_truth = Vec::load_from(path.join("test/travel_time_length"))?;

    let mut gt_iter = ground_truth.iter().map(|&gt| match gt {
        INFINITY => None,
        val => Some(val),
    });

    experiments::run_queries(
        from.iter().copied().zip(to.iter().copied()).take(10000),
        &mut server,
        None,
        |_, _, _| (),
        |_| (),
        |_, _| gt_iter.next(),
    );

    Ok(())
}
