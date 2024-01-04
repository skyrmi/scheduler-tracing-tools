pub mod parser;
use rand::Rng;
use std::collections::HashMap;
use crate::parser::*;
use crate::read_config::{Config, Machine, Graph};
use plotly::common::{ Line, Marker, Mode, Title, MarkerSymbol, HoverInfo};
use plotly::layout::{ Axis, Layout};
use plotly::{Scatter, Plot, ImageFormat, Configuration, Trace};
use plotly::color::{Rgb, NamedColor};

enum ColorTable {
    Command(HashMap<String, Rgb>),
    Parent(HashMap<u32, Rgb>),
    Pid(HashMap<u32, Rgb>),
}

fn random_color() -> Rgb {
    Rgb::new(rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255))
}

fn color_by_pid(actions: &Vec<Action>) -> ColorTable {
    let mut colors: HashMap<u32, Rgb> = HashMap::new();
    for action in actions {
        if let Events::SchedSwitch { old_command: _, old_pid, state: _, new_command: _, new_pid } = &action.event {
            if *old_pid != 0 {
                if let None = colors.get(&old_pid) {
                    colors.insert(*old_pid, random_color());
                }
            }
            if *new_pid != 0 {
                if let None = colors.get(&new_pid) {
                    colors.insert(*new_pid, random_color());
                }
            }
        }
    }
    ColorTable::Pid(colors)
}

fn color_by_command(actions: &Vec<Action>) -> ColorTable {
    let mut colors: HashMap<String, Rgb> = HashMap::new();
    for action in actions {
        if let Events::SchedSwitch { old_command, old_pid, state: _, new_command, new_pid } = &action.event {
            if *old_pid != 0 {
                if let None = colors.get(old_command) {
                    colors.insert(old_command.clone(), random_color());
                }
            }
            if *new_pid != 0 {
                if let None = colors.get(new_command) {
                    colors.insert(new_command.clone(), random_color());
                }
            }
        }
    }
    ColorTable::Command(colors)
}

fn color_by_parent(actions: &Vec<Action>) -> ColorTable {
    let mut colors: HashMap<u32, Rgb> = HashMap::new();
    for action in actions {
        if let Events::SchedProcessFork { pid, child_pid, .. } = &action.event {
            if let None = colors.get(pid) {
                colors.insert(*pid, random_color());
            }
            if let None = colors.get(child_pid) {
                colors.insert(*child_pid, colors[pid]);
            }
        }
        else if let None = colors.get(&action.pid) {
            colors.insert(action.pid, random_color());
        }
    }
    ColorTable::Parent(colors)
}

fn get_socket_order(cpu: u32, machine: &Machine) -> (u32, u32) {
    for (socket_id, socket_ranges) in machine.numa_node_ranges.iter().enumerate() {
        for (range_id, range) in socket_ranges.iter().enumerate() {
            if cpu >= range[0] && cpu <= range[1] {
                return (socket_id as u32, range_id as u32);
            }
        }
    }
    panic!("Bad numa node ranges in config");
}

fn get_y_axis(machine: &Machine, socket_order: bool, cpu_count: u32) -> HashMap<u32, u32> {
    let mut y_axis = HashMap::new();

    if !socket_order {
        for cpu in 0..cpu_count {
            y_axis.insert(cpu, cpu);
        }
        return y_axis;
    }

    for cpu in 0..machine.cpus {
        let (socket, range) = get_socket_order(cpu, machine);
        let cores_per_socket = machine.cores_per_socket;
        let cpu_within_socket = cpu % cores_per_socket;
        let socket_offset = socket * cores_per_socket * machine.threads_per_core;
        let y_axis_cpu_position = socket_offset + cpu_within_socket + range * cores_per_socket;
        y_axis.insert(cpu, y_axis_cpu_position);
    }
    y_axis
}

fn get_sched_switch_events(actions: &Vec<Action>) -> HashMap<u32, Vec<&Action>> {
    let mut data: HashMap<u32, Vec<&Action>> = HashMap::new();
    for action in actions {
        if let Events::SchedSwitch { .. } = &action.event {
            let entry = data.entry(action.cpu).or_insert_with(Vec::new);
            entry.push(action);
        }
    }
    data
}

fn draw_sched_switch(orig: f64, data: HashMap<u32, Vec<&Action>>, color_table: ColorTable, plot: &mut Plot, y_axis: &HashMap<u32, u32>, webgl: bool) {
    for (core, switch_events) in data {
        for item in switch_events.windows(2) {
            if let Events::SchedSwitch { old_command, old_pid, state, new_command, new_pid } = &item[1].event {
                if *old_pid == 0 { continue; }
                
                let hover_text = format!("Timestamp: {}<br>From: {}<br>Pid: {}<br>State: {}<br>To: {}<br>Pid: {}",
                                            item[1].timestamp, old_command, old_pid, state, new_command, new_pid);


                let mut trace = Scatter::new(vec![item[0].timestamp - orig, item[1].timestamp - orig], vec![y_axis[&core], y_axis[&core]])
                    .mode(Mode::Lines)
                    .hover_info(HoverInfo::Skip)   
                    .legend_group("switch")
                    .web_gl_mode(webgl)
                    .show_legend(false);

                let color = match &color_table {
                    ColorTable::Pid(colors) => colors[old_pid],
                    ColorTable::Command(colors) => colors[old_command],
                    ColorTable::Parent(colors) => colors[old_pid]
                };

                trace = trace.line(Line::new().color(color).width(1.0));
                plot.add_trace(trace);

                let trace = Scatter::new(vec![item[1].timestamp - orig], vec![y_axis[&core]])
                        .mode(Mode::Markers)
                        .marker(Marker::new().symbol(MarkerSymbol::LineNSOpen).color(color))
                        .hover_text(hover_text)
                        .name("switch")
                        .legend_group("switch")
                        .web_gl_mode(webgl)
                        .show_legend(false);
                plot.add_trace(trace);
            }
        }
    }
    plot.add_trace(Scatter::new(vec![0, 0], vec![-1, -1])
        .mode(Mode::Markers)
        .marker(Marker::new().symbol(MarkerSymbol::LineEWOpen))
        .legend_group("switch")
        .hover_info(HoverInfo::Skip)
        .web_gl_mode(webgl)
        .name("switch"));
}   

fn draw_migrate_marks(start_time: f64, action: &Action, traces: &mut Vec<Box<dyn Trace>>, legend_group: &str, color: NamedColor, y_axis: &HashMap<u32, u32>, webgl: bool) {
    if let Events::SchedMigrateTask { command, pid, orig_cpu, dest_cpu, .. } = &action.event {

        let trace = Scatter::new(
            vec![action.timestamp - start_time; 2], vec![y_axis[orig_cpu], y_axis[dest_cpu]])
            .mode(Mode::Lines)
            .line(Line::new().color(color).width(1.0))
            .hover_info(HoverInfo::None)
            .legend_group(legend_group)
            .web_gl_mode(webgl)
            .show_legend(false);
        traces.push(trace);

        let hover_text = format!("Timestamp: {}<br>Command: {}<br>Pid: {}<br>Src: {}<br>Dest: {}",
                                    action.timestamp, command, pid, orig_cpu, dest_cpu);

        let mut trace = Scatter::new(
            vec![action.timestamp - start_time], vec![y_axis[dest_cpu]])
            .mode(Mode::Markers)
            .name(legend_group)
            .legend_group(legend_group)
            .hover_text(hover_text)
            .web_gl_mode(webgl)
            .show_legend(false);
        if orig_cpu < dest_cpu {
            trace = trace.marker(Marker::new().color(color).symbol(MarkerSymbol::TriangleUp)
                        .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)));
        } else {
            trace = trace.marker(Marker::new().color(color).symbol(MarkerSymbol::TriangleDown)
            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)));
        }
        traces.push(trace);
    }
}

fn draw_migrate_events(start_time: f64, action: &Action, states: &HashMap<u32, Wstate>, traces: &mut Vec<Box<dyn Trace>>, y_axis: &HashMap<u32, u32>, machine: &Machine, webgl: bool) {
    if let Events::SchedMigrateTask { command: _, pid, orig_cpu, dest_cpu, state: _ } = &action.event {
        let legend_group: &str;
        let color: NamedColor;
        let (src, _) = get_socket_order(*orig_cpu, machine);
        let (dest, _) = get_socket_order(*dest_cpu, machine);

        if states.contains_key(pid) {
            match states[pid] {
                Wstate::Waking(..) => {
                    if src == dest {
                        legend_group = "on-socket<br>unblock placement";
                        color = NamedColor::DeepPink;
                    } 
                    else {
                        legend_group = "off-socket<br>unblock placement";
                        color = NamedColor::SkyBlue;
                    }
                },
                Wstate::Woken => {
                    if src == dest {
                        legend_group = "on-socket<br>load balancing";
                        color = NamedColor::Gold;
                    }
                    else {
                        legend_group = "off-socket<br>load balancing";
                        color = NamedColor::Orange;
                    }
                }
                Wstate::Numa(..) => {
                    legend_group = "numa balancing";
                    color = NamedColor::SeaGreen;
                }
            }
            draw_migrate_marks(start_time, action, traces, legend_group, color, y_axis, webgl);
        }
    }
}

fn draw_legends(plot: &mut Plot, options: &Graph) {
    let marker_legends = vec![("wakeup", NamedColor::RoyalBlue),
                                            ("wakeup new", NamedColor::Brown),
                                            ("wake idle no ipi", NamedColor::LimeGreen),
                                            ("waking", NamedColor::DarkOliveGreen),
                                            ("process fork", NamedColor::Pink)];

    let migrate_legends = vec![("on-socket<br>unblock placement", NamedColor::DeepPink),
                                    ("off-socket<br>unblock placement", NamedColor::SkyBlue),
                                    ("numa balancing", NamedColor::SeaGreen),
                                    ("on-socket<br>load balancing", NamedColor::Gold),
                                    ("off-socket<br>load balancing", NamedColor::Orange)];

    for (name, color) in marker_legends {
        plot.add_trace(Scatter::new(vec![0], vec![-1])
        .mode(Mode::LinesMarkers)
        .marker(Marker::new().color(color).symbol(MarkerSymbol::LineNSOpen))
        .line(Line::new().width(1.0))
        .legend_group(name)
        .hover_info(HoverInfo::Skip)
        .name(name));
    }

    if options.show_events || options.show_migrate {
        for (name, color) in migrate_legends {
            plot.add_trace(Scatter::new(vec![0], vec![-1])
            .mode(Mode::LinesMarkers)
            .marker(Marker::new().color(color).symbol(MarkerSymbol::TriangleRight)
                    .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)))
            .legend_group(name)
            .hover_info(HoverInfo::Skip)
            .name(name));
        }
    }
}

fn add_marker(traces: &mut Vec<Box<dyn Trace>>, action: &Action, start_time: f64, y_axis: &HashMap<u32, u32>, name: &str, hover_text: String, color: NamedColor, options: &Graph) {
    let trace = Scatter::new(
        vec![action.timestamp - start_time], vec![y_axis[&action.cpu]])
        .mode(Mode::Markers)
        .marker(Marker::new().color(color).symbol(MarkerSymbol::LineNSOpen))
        .name(name)
        .legend_group(name)
        .web_gl_mode(options.webgl)
        .hover_text(hover_text)
        .show_legend(false);
    traces.push(trace);
}

fn draw_traces(filepath: &str, config: &Config, plot: &mut Plot) -> TraceParser {
    let mut reader = TraceParser::new(filepath);
    let mut switch_events: Vec<Action> = Vec::new();
    let mut boundary_events: HashMap<u32, Action> =  HashMap::new();
    let mut fork_events: Vec<Action> = Vec::new();
    let mut traces: Vec<Box<dyn Trace>> = Vec::new();
    let mut migrate_traces: Vec<Box<dyn Trace>> = Vec::new();

    let options = &config.graph;
    let y_axis = get_y_axis(&config.machine, options.socket_order, reader.cpu_count);

    while let Some((action, states, Some(start_time))) = reader.next_action() {
        // collect the switch events going through the boundary of the range
        if options.custom_range {
            if action.timestamp - start_time < options.min {
                if let Events::SchedSwitch { .. } = action.event {
                    boundary_events.insert(action.cpu, action);
                }
                continue;
            }
            else if action.timestamp - start_time > options.max {
                if boundary_events.len() < reader.cpu_count.try_into().unwrap() {
                    if let Events::SchedSwitch { .. } = action.event {
                        if let None = boundary_events.get(&action.cpu) {
                            boundary_events.insert(action.cpu, action);
                        }
                    }
                    continue;
                } else {
                    for (_, v) in boundary_events.drain() {
                        switch_events.push(v);
                    }
                    break;
                }
            }
        }
        
        match &action.event {
            Events::SchedSwitch { .. } => {
                if options.custom_range && !boundary_events.is_empty()  {
                    for (_, v) in boundary_events.drain() {
                        switch_events.push(v);
                    }
                }
                switch_events.push(action);
            },
            Events::SchedWakeup { command, pid, prev_cpu, cpu } => {
                let name = "wakeup";
                let hover_text = format!("Timestamp: {}<br>Waker: {}<br>Waker pid: {}<br>Wakee: {}<br>Wakee pid: {}",
                                action.timestamp, action.process, action.pid, command, pid);
                let color = NamedColor::RoyalBlue;
                add_marker(&mut traces, &action, start_time, &y_axis, name, hover_text, color, options);
            },
            Events::SchedWakeupNew { command, pid, parent_cpu, cpu } => {
                let name = "wakeup new";
                let hover_text = format!("Timestamp: {}<br>Command: {}<br>Waker pid: {}<br>Wakee pid: {}<br>Target cpu: {}",
                                action.timestamp, action.process, action.pid, pid, cpu);
                let color = NamedColor::Brown;
                add_marker(&mut traces, &action, start_time, &y_axis, name, hover_text, color, options);
            },
            Events::SchedWakeIdleNoIpi { cpu } => {
                let name = "wake idle no ipi";
                let hover_text = format!("Timestamp: {}<br>Command: {}<br>Pid: {}", action.timestamp, action.process, action.pid);
                let color = NamedColor::LimeGreen;
                add_marker(&mut traces, &action, start_time, &y_axis, name, hover_text, color, options);
            }
            Events::SchedWaking { command, pid, target_cpu } => {
                let name = "waking";
                let hover_text = format!("Timestamp: {}<br>Command: {}<br>Waker pid: {}<br>Wakee pid: {}<br>Target cpu: {}",
                                action.timestamp, action.process, action.pid, pid, target_cpu);
                let color = NamedColor::DarkOliveGreen;
                add_marker(&mut traces, &action, start_time, &y_axis, name, hover_text, color, options);
            },
            Events::SchedProcessFork { command, pid, child_command, child_pid } => {
                let name = "process fork";
                let hover_text = format!("Timestamp: {}<br>Command: {}<br>Pid: {}<br>Child command: {}<br>Child pid: {}",
                                action.timestamp, command, pid, child_command, child_pid);
                let color = NamedColor::Pink;
                add_marker(&mut traces, &action, start_time, &y_axis, name, hover_text, color, options);
                fork_events.push(action);
            }
            Events::SchedMigrateTask { .. } => {
                draw_migrate_events(start_time, &action, states, &mut migrate_traces, &y_axis, &config.machine, options.webgl);
            }
            _ => { }
        }
    }

    let color_table = match options.color_by.as_str() {
        "pid" => color_by_pid(&switch_events),
        "command" => color_by_command(&switch_events),
        "parent" => color_by_parent(&fork_events),
        _ => { panic!("Invalid color option"); }
    };

    let switch_events = get_sched_switch_events(&switch_events);
    draw_sched_switch(reader.first_timestamp.unwrap(), switch_events, color_table, plot, &y_axis, options.webgl);

    if options.show_events || options.show_wake {
        plot.add_traces(traces);
    }
    if options.show_events || options.show_migrate {
        plot.add_traces(migrate_traces);
    }
    draw_legends(plot, options);
    reader
}

pub fn data_graph(filepath: &str, config: &Config) {
    let options = &config.graph;
    let filename = filepath.split("/").last().unwrap();
    let mut plot = Plot::new();

    let reader = draw_traces(filepath, config, &mut plot);
    
    let duration: Vec<f64>;
    if options.custom_range {
        duration = vec![options.min, options.max];
    } else {
        duration = vec![0.0, reader.last_timestamp.unwrap() - reader.first_timestamp.unwrap()];
    }
    let layout = Layout::new()
                                .title(Title::new(format!("Data Graph: {}", filename).as_str()))
                                .x_axis(
                                    Axis::new()
                                    .title(Title::new("Duration (seconds)"))
                                    .range(duration)
                                    .show_grid(false))
                                .y_axis(
                                    Axis::new()
                                    .title(Title::new("Cores"))
                                    .range(vec![0, reader.cpu_count - 1])
                                    .show_grid(false))
                                .auto_size(true);

    
    plot.set_configuration(Configuration::display_logo(plot.configuration().clone(), false));
    plot.set_configuration(Configuration::fill_frame(plot.configuration().clone(), true));

    if !options.interactive {
        plot.set_configuration(Configuration::static_plot(plot.configuration().clone(), true));
    }

    plot.set_layout(layout);
    plot.use_local_plotly();
    if options.launch_default_browser {
        plot.show();
    }

    // plot.write_html(format!("{}{}.html", options.output_path, filename));
    if options.gen_static {
        let image_format = match options.filetype.as_str() {
            "png" => ImageFormat::PNG,
            "svg" => ImageFormat::SVG,
            "jpeg" => ImageFormat::JPEG,
            "webp" => ImageFormat::WEBP,
            "pdf" => ImageFormat::PDF,
            "eps" => ImageFormat::EPS,
            _ => { panic!("Invalid static file format"); }
        };
        plot.write_image(format!("{}{}.{}", options.output_path, filename, options.filetype), image_format, options.static_res_width, options.static_res_height, 1.0);
    }
}
