use std::fs::read_to_string;
use serde::Deserialize;
use std::fs::File;
use std::io::Write;
use clap_serde_derive::{
    clap::{self},
    serde::Serialize,
    ClapSerde,
};

#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone)]
pub struct Machine {
    pub cpus: u32,
    pub sockets: u32,
    pub cores_per_socket: u32,
    pub threads_per_core: u32,
    pub numa_nodes: u32,
    pub numa_node_ranges: Vec<Vec<Vec<u32>>>,
}


#[derive(ClapSerde, Serialize, Deserialize)]
#[derive(Debug, Clone)]
#[command(about = "Visualize trace-cmd report")]
pub struct Graph {
    #[arg(long, default_value = "pid", required = false, value_names = ["pid", "parent", "command"])]
    pub color_by: String,

    #[arg(long, required = false)]
    pub socket_order: bool,

    #[arg(long, required = false)]
    pub interactive: bool,

    #[arg(long, required = false)]
    pub webgl: bool,

    #[arg(long, required = false)]
    pub gen_static: bool,

    #[default(1920)]
    #[arg(long, required = false)]
    pub static_res_width: usize,

    #[default(1080)]
    #[arg(long, required = false)]
    pub static_res_height: usize,

    #[arg(long, required = false)]
    pub launch_default_browser: bool,

    #[arg(long, default_value = "", required = false)]
    pub output_path: String,

    #[arg(long, default_value = "png", required = false, value_names = ["png", "svg", "webp", "jpeg", "pdf", "eps"])]
    pub filetype: String,

    #[arg(long, required = false)]
    pub show_switch: bool,

    #[arg(long, required = false)]
    pub show_events: bool,

    #[arg(long, required = false)]
    pub show_all: bool,

    #[arg(long, required = false)]
    pub show_wake: bool,

    #[arg(long, required = false)]
    pub show_migrate: bool,

    #[arg()]
    pub files: Vec<String>
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
pub struct Config {
    pub machine: Machine,
    pub graph: Graph,
}

pub fn config() -> Config {
    let mut temp_str = read_to_string("./config.toml");
    if temp_str.is_err() {
        let mut writer = File::create("./config.toml").expect("Failed to generate config");
        writer.write_all(default_config().as_bytes()).expect("Error while writing config");
        temp_str = read_to_string("./config.toml");
    }
    let config_str = temp_str.unwrap();
    let Config {machine, graph}: Config = toml::from_str(&config_str).expect("Failed to parse config");
    let graph = graph.merge_clap();
    let config = Config { machine, graph };
    config
}

pub fn default_config() -> String {
    String::from("[machine]
    cpus = 64
    sockets = 2
    cores_per_socket = 16
    threads_per_core = 2
    numa_nodes = 2
    numa_node_ranges = [ 
                            [ 
                                [0, 15], 
                                [32, 47] 
                            ], 
                            [ 
                                [16, 31],
                                [48, 63]
                            ] 
                        ]


[graph]
    # color options: pid, command, parent
    color_by = \"parent\"

    # if true cpus are arranged as per sockets
    socket_order = false

    # set graph's interactivity
    interactive = true

    # webgl improves performance especially for large graphs, but may cause pixelation
    webgl = false

    # static graph generation other than html and size 
    gen_static = false
    static_res_width = 1920
    static_res_height = 1080

    # whether to launch the system default browser to view the generated html file
    launch_default_browser = true
    
    # non-empty path should end with '/'
    output_path = \"\"

    # static filetype options = png, jpeg, webp, svg, pdf, eps
    filetype = \"png\"

    # whether sched_switch events are shown, is unaffected by show_events options
    show_switch = true

    # choose which events to show, if show_events = true
    show_events = true
    show_all = true
    show_wake =  false
    show_migrate = false

    # input files, can be given as an array here or via commmand line arguments
    files = [\"\"]"
)
}