pub mod graph;
pub mod read_config;

use graph::*;
use read_config::config;
use std::env;

fn main() {
    let config = config();
    let args: Vec<_> = env::args().collect();
    for arg in args.iter().skip(1) {
        data_graph(arg, &config);
    }
}