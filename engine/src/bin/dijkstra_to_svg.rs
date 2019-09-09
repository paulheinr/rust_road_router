use std::env;
use std::path::Path;

use bmw_routing_engine::{
    graph::*,
    io::Load,
    shortest_path::{
        Query,
        stepped_dijkstra::*,
    },
};

fn main() {
    let mut args = env::args();
    args.next();

    let arg = &args.next().expect("No directory arg given");
    let path = Path::new(arg);

    let min_lat = args.next().map(|s| s.parse::<f32>().unwrap_or_else(|_| panic!("could not parse {} as lat coord", s))).unwrap();
    let min_lon = args.next().map(|s| s.parse::<f32>().unwrap_or_else(|_| panic!("could not parse {} as lon coord", s))).unwrap();
    let max_lat = args.next().map(|s| s.parse::<f32>().unwrap_or_else(|_| panic!("could not parse {} as lat coord", s))).unwrap();
    let max_lon = args.next().map(|s| s.parse::<f32>().unwrap_or_else(|_| panic!("could not parse {} as lon coord", s))).unwrap();

    let start_lat = args.next().map(|s| s.parse::<f32>().unwrap_or_else(|_| panic!("could not parse {} as lat coord", s))).unwrap();
    let start_lon = args.next().map(|s| s.parse::<f32>().unwrap_or_else(|_| panic!("could not parse {} as lon coord", s))).unwrap();

    let first_out = Vec::load_from(path.join("first_out").to_str().unwrap()).expect("could not read first_out");
    let head = Vec::load_from(path.join("head").to_str().unwrap()).expect("could not read head");
    let travel_time = Vec::load_from(path.join("travel_time").to_str().unwrap()).expect("could not read travel_time");
    let lat = Vec::<f32>::load_from(path.join("latitude").to_str().unwrap()).expect("could not read latitude");
    let lng = Vec::<f32>::load_from(path.join("longitude").to_str().unwrap()).expect("could not read longitude");

    let in_bounding_box = |node| {
        lat[node] >= min_lat && lat[node] <= max_lat && lng[node] >= min_lon && lng[node] <= max_lon
    };

    let graph = FirstOutGraph::new(&first_out[..], &head[..], &travel_time[..]);

    println!("<svg version=\"1.1\" xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" viewBox=\"{} {} {} {}\" style=\"transform: scale(1,-1);\" preserveAspectRatio=\"none\">", min_lon, min_lat, max_lon - min_lon, max_lat - min_lat);
    println!("<g>");

    let mut min_dist = std::f32::INFINITY;
    let mut start_node = 0;

    for node in 0..graph.num_nodes() {
        if in_bounding_box(node) {
            let delta_lat = lat[node] - start_lat;
            let delta_lng = lng[node] - start_lon;
            let dist = delta_lat * delta_lat + delta_lng * delta_lng;
            if dist < min_dist {
                min_dist = dist;
                start_node = node;
            }

            println!("<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"node\" />", lng[node], lat[node], lng[node], lat[node]);
            for link in graph.neighbor_iter(node as NodeId) {
                if in_bounding_box(link.node as usize) {
                    println!("<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"arc\" />", lng[node], lat[node], lng[link.node as usize], lat[link.node as usize]);
                }
            }
        }
    }
    println!("</g>");

    let mut dijkstra = SteppedDijkstra::new(graph.clone());
    dijkstra.initialize_query(Query { from: start_node as NodeId, to: std::u32::MAX });

    let mut counter = 0;
    while let QueryProgress::Progress(State { node, .. }) = dijkstra.next_step() {
        if counter > 500 { break; }
        counter += 1;
        if in_bounding_box(node as usize) {
            print!("<g class=\"settled fragment\"");
            if counter <= 500 { print!(" data-autoslide=\"25\""); }
            println!(">");
            println!("<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"node\" />", lng[node as usize], lat[node as usize], lng[node as usize], lat[node as usize]);
            for link in graph.neighbor_iter(node) {
                if in_bounding_box(link.node as usize) {
                    println!("<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"arc\" />", lng[node as usize], lat[node as usize], lng[link.node as usize], lat[link.node as usize]);
                }
            }
            println!("</g>");
        }
    }

    println!("</svg>");
}
