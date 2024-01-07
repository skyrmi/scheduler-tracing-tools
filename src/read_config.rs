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
    /// Available color options: pid, command, parent
    #[arg(long, default_value = "pid", required = false)]
    pub color_by: String,

    /// Whether cpus in the same socket should be grouped together
    #[arg(long, required = false)]
    pub socket_order: bool,
    
    /// Start plot after first sleep command
    #[arg(long, required = false)]
    pub sleep: bool,

    /// Show trace file name as title on top of graph
    #[arg(long, required = false)]
    pub show_title: bool,

    /// Whether to create a html plot
    #[arg(long, required = false)]
    pub create_html: bool,

    /// Whether the generated html is interactive
    #[arg(long, required = false)]
    pub interactive: bool,

    /// Transparent markers to display when hovering on a line
    #[arg(long, required = false)]
    pub line_marker_count: u32,

    /// Ignore switch events smaller than limit when not interative
    #[arg(long, required = false)]
    pub limit: f64,

    /// Webgl improves performance but may cause pixelation
    #[arg(long, required = false)]
    pub webgl: bool,

    /// To select a portion of the trace to plot
    #[arg(long, required = false)]
    pub custom_range: bool,

    /// the lower limit of the displayed range
    #[arg(long, required = false)]
    pub min: f64,

    /// the higher limit of the displayed range
    #[arg(long, required = false)]
    pub max: f64,

    /// Whether to show the generated plot
    #[arg(long, required = false)]
    pub show_html: bool,

    /// Browser program name, will use default if empty
    #[arg(long, required = false)]
    pub browser: String,

    /// Output location for the plots, default is current directory
    #[arg(long, default_value = "", required = false)]
    pub output_path: String,

    /// Options for static plot other than html
    #[clap_serde]
    #[command(flatten)]
    pub static_options: Static,

    /// Events to show
    #[clap_serde]
    #[command(flatten)]
    pub events: Events,

    #[arg()]
    pub files: Vec<String>
}

#[derive(ClapSerde, Serialize, Deserialize)]
#[derive(Debug, Clone)]
pub struct Static {
    /// Whether to generate a static plot other than the html
    #[arg(long, required = false)]
    pub gen_static: bool,

    /// Width of the static plot
    #[default(1920)]
    #[arg(long, required = false)]
    pub static_res_width: usize,

    /// Height of the static plot
    #[default(1080)]
    #[arg(long, required = false)]
    pub static_res_height: usize,

    /// Filetype of the static plot, available options: png, svg, webp, pdf, jpeg, eps
    #[arg(long, default_value = "png", required = false)]
    pub filetype: String,
}

#[derive(ClapSerde, Serialize, Deserialize)]
#[derive(Debug, Clone)]
#[command()]
pub struct Events {
    /// Whether to show all events
    #[arg(long, required = false)]
    pub show_events: bool,

    /// Toggle for switch event notches
    #[arg(long, required = false)]
    pub show_switch: bool,

    /// Events represented with only a notch: wake, process fork/exec etc.
    #[arg(long, required = false)]
    pub show_marker_only: bool,

    /// Migration events: unblock placement, load balancing, numa balancing
    #[arg(long, required = false)]
    pub show_migrate: bool,
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
pub struct Config {
    pub machine: Machine,
    pub graph: Graph,
}


// Priority order for config options:
// Command line arguments > config file options > defaults (if present)
pub fn config() -> Config {
    let mut temp_str = read_to_string("./tracing-tool-config.toml");
    if temp_str.is_err() {
        let mut writer = File::create("./tracing-tool-config.toml").expect("Failed to generate config");
        writer.write_all(default_config().as_bytes()).expect("Error while writing config");
        temp_str = read_to_string("./tracing-tool-config.toml");
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

    # start plot after the first sleep command
    sleep = false

    # whether to have filename as title on top of graph
    show_title = true

    # whether to create a html plot
    create_html = true

    # set the html plot's interactivity
    interactive = true

    # transparent marker count for hover info between switch events
    line_marker_count = 0
    
    # Switch events smaller than limit will be ignored if not interactive
    limit = 0.0

    # webgl improves performance especially for large graphs, but may cause pixelation
    webgl = false

    # whether to show only a part of the graph
    custom_range = false

    # bounds for the part to show
    min = 0.0
    max = 0.0

    # whether to show the generated html file after creation
    show_html = true

    # browser program name, if empty default is used
    browser = \"\"
    
    # Location for the generated file(s)
    output_path = \"\"

    # input files, can be given as an array here or via commmand line arguments
    files = [\"\"]

[graph.events]
    # choose which events to show, all are shown if show_events = true
    show_events = true

    # toggle for switch event notches
    show_switch = false

    # Events represented with only a notch: wake, process fork/exec etc.
    show_marker_only =  false

    # Migration events: unblock placement, load balancing, numa balancing
    show_migrate = false

[graph.static_options]
    # generate static graph in a different file format
    gen_static = false

    static_res_width = 1920
    static_res_height = 1080

    # filetype options = png, jpeg, webp, svg, pdf, eps
    filetype = \"png\"
"
)
}