use ::graph::*;
use ::graph::first_out_graph::FirstOutGraph;
use ::rank_select_map::RankSelectMap;
use std::iter;
use std::str::FromStr;
use std::error::Error;
use std::fmt;

use nav_types::WGS84;

pub mod postgres_source;
pub mod csv_source;

#[derive(Debug)]
pub struct DirectionParseError;

impl fmt::Display for DirectionParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Here direction could not be parsed!")
    }
}

impl Error for DirectionParseError {
    fn description(&self) -> &str {
        "Here direction could not be parsed!"
    }
}

#[derive(Debug)]
pub enum RdfLinkDirection {
    FromRef,
    ToRef,
    Both
}

impl FromStr for RdfLinkDirection {
    type Err = DirectionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.chars().next() {
            Some('B') => Ok(RdfLinkDirection::Both),
            Some('F') => Ok(RdfLinkDirection::FromRef),
            Some('T') => Ok(RdfLinkDirection::ToRef),
            _ => Err(DirectionParseError)
        }
    }
}

#[derive(Debug)]
pub struct RdfLink {
    link_id: i64,
    ref_node_id: i64,
    nonref_node_id: i64
}

#[derive(Debug)]
pub struct RdfNavLink {
    link_id: i64,
    travel_direction: RdfLinkDirection,
    speed_category: i32,
    from_ref_speed_limit: Option<i32>,
    to_ref_speed_limit: Option<i32>
}

impl RdfNavLink {
    fn speed_in_m_per_s(&self, direction: RdfLinkDirection) -> f64 {
        let link_speed = match self.speed_category {
            1 => 36.11,
            2 => 31.94,
            3 => 26.388,
            4 => 22.22,
            5 => 16.66,
            6 => 11.11,
            7 => 5.55,
            8 => 1.38,
            _ => panic!("unknown speed category")
        };

        let limit = match direction {
            RdfLinkDirection::FromRef => self.from_ref_speed_limit,
            RdfLinkDirection::ToRef => self.to_ref_speed_limit,
            _ => panic!("invalid argument")
        };

        let limit = limit.unwrap_or(999) as f64 / 3.6;

        if limit < link_speed { limit } else { link_speed }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RdfNode {
    node_id: i64,
    lat: i64,
    lon: i64,
    z_coord: Option<i64>
}

impl RdfNode {
    fn as_wgs84(&self) -> WGS84<f64> {
        WGS84::new(
            (self.lat as f64) / 100000.,
            (self.lon as f64) / 100000.,
            (self.z_coord.unwrap_or(0) as f64) / 100.)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RdfLinkGeometry {
    link_id: i64,
    seq_num: i64,
    lat: i64,
    lon: i64,
    z_coord: Option<i64>
}

impl RdfLinkGeometry {
    fn as_wgs84(&self) -> WGS84<f64> {
        WGS84::new(
            (self.lat as f64) / 100000.,
            (self.lon as f64) / 100000.,
            (self.z_coord.unwrap_or(0) as f64) / 100.)
    }
}

pub trait RdfDataSource {
    fn links(&self) -> Vec<RdfLink>;
    fn nav_links(&self) -> Vec<RdfNavLink>;
    fn nodes(&self) -> Vec<RdfNode>;
    fn link_geometries(&self) -> Vec<RdfLinkGeometry>;
}

pub fn read_graph(source: &RdfDataSource) -> FirstOutGraph {
    println!("read nav links");
    // start with all nav links
    let mut nav_links: Vec<RdfNavLink> = source.nav_links();
    println!("sort nav links");
    nav_links.sort_by_key(|nav_link| nav_link.link_id);

    println!("build link indices");
    // local ids for links
    let mut link_indexes = RankSelectMap::new(nav_links.last().unwrap().link_id as usize + 1);
    for nav_link in nav_links.iter() {
        link_indexes.insert(nav_link.link_id as usize);
    }
    link_indexes.compile();

    println!("read links");
    let links = source.links();
    let maximum_node_id = links.iter().flat_map(|link| iter::once(link.ref_node_id).chain(iter::once(link.nonref_node_id)) ).max().unwrap();

    // a data structure to do the global to local node ids mapping
    let mut node_id_mapping = RankSelectMap::new(maximum_node_id as usize + 1);


    println!("build node id mapping");
    // insert all global node ids we encounter in links
    for link in links.iter() {
        match link_indexes.get(link.link_id as usize) {
            Some(_) => {
                node_id_mapping.insert(link.ref_node_id as usize);
                node_id_mapping.insert(link.nonref_node_id as usize);
            },
            None => (),
        }
    }
    node_id_mapping.compile();

    println!("build up degrees");
    // now we know the number of nodes
    let n = node_id_mapping.len();
    // vector to store degrees which will then be turned into the first_out array
    let mut degrees: Vec<u32> = vec![0; n + 1];

    // iterate over all links and count degrees for nodes
    for link in links.iter() {
        match link_indexes.get(link.link_id as usize) {
            Some(link_index) => {
                match nav_links[link_index].travel_direction {
                    RdfLinkDirection::FromRef => degrees[node_id_mapping.at(link.ref_node_id as usize)] += 1,
                    RdfLinkDirection::ToRef => degrees[node_id_mapping.at(link.nonref_node_id as usize)] += 1,
                    RdfLinkDirection::Both => {
                        degrees[node_id_mapping.at(link.ref_node_id as usize)] += 1;
                        degrees[node_id_mapping.at(link.nonref_node_id as usize)] += 1;
                    }
                }
            },
            None => (),
        }
    }

    // do prefix sum over degrees which yields first_out
    // doing in place mutation and afterwards move into new var here
    // if we'd use into_iter the compiler might figure out to do this as well, but I dont know
    let m = degrees.iter_mut().fold(0, |prefix, degree| {
        let sum = prefix + *degree;
        *degree = prefix;
        sum
    });
    let mut first_out = degrees; // move

    println!("read link geometry");
    // fetch links geometry
    let mut link_geometries = vec![Vec::new(); link_indexes.len()];
    for geometry in source.link_geometries().into_iter() {
        match link_indexes.get(geometry.link_id as usize) {
            Some(link_index) => link_geometries[link_index].push(geometry),
            None => (),
        }
    }
    for geometries in link_geometries.iter_mut() {
        geometries.sort_by_key(|geometry| geometry.seq_num);
    }

    println!("read nodes");
    let mut nodes: Vec<RdfNode> = vec![RdfNode { node_id: 0, lat: 0, lon: 0, z_coord: None }; n];
    for node in source.nodes().into_iter() {
        match node_id_mapping.get(node.node_id as usize) {
            Some(index) => nodes[index] = node,
            None => (),
        }
    }

    // init other graph arrays
    let mut head: Vec<NodeId> = vec![0; m as usize];
    let mut weights: Vec<Weight> = vec![0; m as usize];

    println!("calculate weights");
    // iterate over all links and insert head and weight
    // increment the first_out values along the way
    for link in links.iter() {
        match link_indexes.get(link.link_id as usize) {
            Some(link_index) => {
                let nav_link = &nav_links[link_index];
                let length = if link_geometries[link_index].is_empty() {
                    let head = &nodes[node_id_mapping.at(link.ref_node_id as usize)];
                    let tail = &nodes[node_id_mapping.at(link.nonref_node_id as usize)];
                    head.as_wgs84().distance(&tail.as_wgs84())
                } else {
                    calculate_length_in_m(&link_geometries[link_index])
                };
                let from_weight = (1000. * length / nav_link.speed_in_m_per_s(RdfLinkDirection::FromRef)).round() as Weight;
                let to_weight = (1000. * length / nav_link.speed_in_m_per_s(RdfLinkDirection::ToRef)).round() as Weight;

                match nav_link.travel_direction {
                    RdfLinkDirection::FromRef => {
                        head[first_out[node_id_mapping.at(link.ref_node_id as usize)] as usize] = node_id_mapping.at(link.nonref_node_id as usize) as NodeId;
                        weights[first_out[node_id_mapping.at(link.ref_node_id as usize)] as usize] = from_weight;
                        first_out[node_id_mapping.at(link.ref_node_id as usize)] += 1;
                    },
                    RdfLinkDirection::ToRef => {
                        head[first_out[node_id_mapping.at(link.nonref_node_id as usize)] as usize] = node_id_mapping.at(link.ref_node_id as usize) as NodeId;
                        weights[first_out[node_id_mapping.at(link.nonref_node_id as usize)] as usize] = to_weight;
                        first_out[node_id_mapping.at(link.nonref_node_id as usize)] += 1;
                    },
                    RdfLinkDirection::Both => {
                        head[first_out[node_id_mapping.at(link.ref_node_id as usize)] as usize] = node_id_mapping.at(link.nonref_node_id as usize) as NodeId;
                        weights[first_out[node_id_mapping.at(link.ref_node_id as usize)] as usize] = from_weight;
                        first_out[node_id_mapping.at(link.ref_node_id as usize)] += 1;

                        head[first_out[node_id_mapping.at(link.nonref_node_id as usize)] as usize] = node_id_mapping.at(link.ref_node_id as usize) as NodeId;
                        weights[first_out[node_id_mapping.at(link.nonref_node_id as usize)] as usize] = to_weight;
                        first_out[node_id_mapping.at(link.nonref_node_id as usize)] += 1;
                    }
                }
            },
            None => (),
        }
    }

    // first out values got basically incremented to the value of their successor node
    // pop the last one (wasn't changed)
    first_out.pop().unwrap();
    // insert a zero at the beginning - this will shift all values one to the right
    first_out.insert(0, 0);

    FirstOutGraph::new(first_out, head, weights)
}

fn calculate_length_in_m(geometries: &[RdfLinkGeometry]) -> f64 {
    geometries
        .windows(2)
        .map(|pair| pair[0].as_wgs84().distance(&pair[1].as_wgs84()))
        .sum()
}