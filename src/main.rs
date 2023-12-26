pub mod graph;
pub mod read_config;

use std::process::Command;
use std::fs::File;
use std::fs::remove_file;
use std::io::Write;
use graph::*;
use read_config::{config, Config};
use std::env;

fn main() {
    let config = config();
    let args: Vec<_> = env::args().collect();
    for arg in args.iter().skip(1) {
        make_graph(arg, &config);
    }
}

fn make_graph(filepath: &String, config:&Config) {
    let filepath = filepath;
    let filename = filepath.split("/").last().unwrap();

    let trace_name: String;
    if let Some((name, "dat")) = filename.rsplit_once(".") {
        let output = Command::new("trace-cmd")
                .arg("report")
                .arg(filepath)
                .output()
                .expect("Trace-cmd failed on dat file");
        
        trace_name = format!("{}.txt", name);
        let mut writer = File::create(trace_name.clone()).expect("Failed to create trace");
        writer.write_all(&output.stdout).expect("Error while writing trace");

        data_graph(&trace_name, config);

        remove_file(&trace_name).expect("couldn't remove generated trace file");
    }
    else {
        data_graph(filepath, config);
    }
}