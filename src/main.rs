pub mod graph;

use graph::*;
use std::env;

fn main() {
    let args: Vec<_> = env::args().collect();
    data_graph(&args[1], true);
    
}