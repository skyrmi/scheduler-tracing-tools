use std::fs::read_to_string;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Machine {
    pub cpus: u32,
    pub socket_order: bool,
    pub sockets: u32,
    pub cores_per_socket: u32,
    pub threads_per_core: u32,
    pub numa_nodes: u32,
    pub numa_node_ranges: Vec<Vec<Vec<u32>>>,
}

#[derive(Debug, Deserialize)]
pub struct Graph {
    pub color_by: String,
    pub interactive: bool,
    pub gen_static: bool,
    pub static_res_width: usize,
    pub static_res_height: usize,
    pub launch_default_browser: bool,
    pub filetype: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub machine: Machine,
    pub graph: Graph,
}

pub fn config() -> Config {
    let config_str = read_to_string("./config.toml").expect("Failed to read config");
    let config: Config = toml::from_str(&config_str).expect("Failed to parse config");

    println!("{:?}", config.machine.numa_node_ranges);
    config
}