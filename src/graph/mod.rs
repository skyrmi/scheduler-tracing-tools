pub mod parser;
use rand::Rng;
use std::collections::{HashMap, HashSet};
use crate::parser::*;
use crate::read_config::{Config, Machine};
use plotly::common::{ Line, Marker, Mode, Title, MarkerSymbol, HoverInfo};
use plotly::layout::{ Axis, Layout};
use plotly::{Scatter, Plot, ImageFormat, Configuration};
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
        if let Events::SchedSwitch { old_base, state: _, new_base } = &action.event {
            if old_base.pid != 0 {
                if let None = colors.get(&old_base.pid) {
                    colors.insert(old_base.pid, random_color());
                }
            }
            if new_base.pid != 0 {
                if let None = colors.get(&new_base.pid) {
                    colors.insert(new_base.pid, random_color());
                }
            }
        }
    }
    ColorTable::Pid(colors)
}

fn color_by_command(actions: &Vec<Action>) -> ColorTable {
    let mut colors: HashMap<String, Rgb> = HashMap::new();
    for action in actions {
        if let Events::SchedSwitch { old_base, state: _, new_base } = &action.event {
            if old_base.pid != 0 {
                if let None = colors.get(&old_base.command) {
                    colors.insert(old_base.command.clone(), random_color());
                }
            }
            if new_base.pid != 0 {
                if let None = colors.get(&new_base.command) {
                    colors.insert(new_base.command.clone(), random_color());
                }
            }
        }
    }
    ColorTable::Command(colors)
}

fn color_by_parent(actions: &Vec<Action>) -> ColorTable {
    let mut parent_child_map: HashMap<u32, HashSet<u32>> = HashMap::new();
    for action in actions {
        if let Events::SchedProcessFork { pid, child_pid, .. } = &action.event {
            let children = parent_child_map.entry(*pid).or_insert_with(HashSet::new);
            children.insert(*child_pid);
        }
    }

    let mut colors: HashMap<u32, Rgb> = HashMap::new();
    for (_, children) in parent_child_map.iter() {
        let color = random_color();
        for child in children {
            if let None = colors.get(child) {
                colors.insert(*child, color);
            }
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

fn get_y_axis(machine: &Machine) -> HashMap<u32, u32> {
    let mut y_axis = HashMap::new();

    if !machine.cores_in_socket_order {
        for cpu in 0..machine.cpus {
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
            if let Events::SchedSwitch { old_base, state, new_base} = &item[1].event {
                if old_base.pid == 0 { continue; }
                let action = item[1];
                
                let hover_text = format!("Timestamp: {}<br>From: {}<br>Pid: {}<br>State: {}<br>To: {}<br>Pid: {}",
                                            action.timestamp, old_base.command, old_base.pid, state, new_base.command, new_base.pid);

                let mut trace = Scatter::new(vec![item[0].timestamp - orig, item[1].timestamp - orig], vec![y_axis[&core], y_axis[&core]])
                    .mode(Mode::LinesMarkers)
                    .marker(Marker::new().symbol(MarkerSymbol::LineNSOpen))
                    .hover_text(hover_text)   
                    .name("switch")
                    .legend_group("switch")
                    .web_gl_mode(webgl)
                    .show_legend(false);

                match &color_table {
                    ColorTable::Pid(colors) => {
                        trace = trace.line(Line::new().color(colors[&old_base.pid]).width(1.0));
                    }
                    ColorTable::Command(colors) => {
                        trace = trace.line(Line::new().color(colors[&old_base.command]).width(1.0));
                    }
                    _ => {}
                }

                plot.add_trace(trace);
            }
        }
    }
}   

fn draw_wakeup(actions: &Vec<Action>, plot: &mut Plot, y_axis: &HashMap<u32, u32>, webgl: bool) {
    let orig = actions.first().unwrap().timestamp;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u32> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    
    for action in actions {
        if let Events::SchedWakeup { base, .. } = &action.event {
            xs.push(action.timestamp - orig);
            ys.push(y_axis[&action.cpu]);
            labels.push(format!("Timestamp: {}<br>Waker: {}<br>Waker pid: {}<br>Wakee: {}<br>Wakee pid: {}",
                            action.timestamp, action.process, action.pid, base.command, base.pid));
        }
    }

    let trace = Scatter::new(
        xs, ys)
        .mode(Mode::Markers)
        .marker(Marker::new().color(NamedColor::RoyalBlue).symbol(MarkerSymbol::LineNSOpen))
        .name("wakeup")
        .web_gl_mode(webgl)
        .hover_text_array(labels);
    plot.add_trace(trace);
}

fn draw_wakeup_new(actions: &Vec<Action>, plot: &mut Plot, y_axis: &HashMap<u32, u32>, webgl: bool) {
    let orig = actions.first().unwrap().timestamp;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u32> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    
    for action in actions {
        if let Events::SchedWakeupNew { base, parent_cpu: _ , cpu } = &action.event {
            xs.push(action.timestamp - orig);
            ys.push(y_axis[&action.cpu]);
            labels.push(format!("Timestamp: {}<br>Command: {}<br>Waker pid: {}<br>Wakee pid: {}<br>Target cpu: {}",
                            action.timestamp, action.process, action.pid, base.pid, cpu));
        }
    }

    let trace = Scatter::new(
        xs, ys)
        .mode(Mode::Markers)
        .marker(Marker::new().color(NamedColor::Brown).symbol(MarkerSymbol::LineNSOpen))
        .name("wakeup new")
        .web_gl_mode(webgl)
        .hover_text_array(labels);
    plot.add_trace(trace);
}

fn draw_wakeup_no_ipi(actions: &Vec<Action>, plot: &mut Plot, y_axis: &HashMap<u32, u32>, webgl: bool) {
    let orig = actions.first().unwrap().timestamp;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u32> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    
    for action in actions {
        if let Events::SchedWakeIdleNoIpi { .. } = &action.event {
            xs.push(action.timestamp - orig);
            ys.push(y_axis[&action.cpu]);
            labels.push(format!("Timestamp: {}<br>Command: {}<br>Pid: {}", action.timestamp, action.process, action.pid));
        }
    }

    let trace = Scatter::new(
        xs, ys)
        .mode(Mode::Markers)
        .marker(Marker::new().color(NamedColor::LimeGreen).symbol(MarkerSymbol::LineNSOpen))
        .name("wake idle without ipi")
        .web_gl_mode(webgl)
        .hover_text_array(labels);
    plot.add_trace(trace);
}

fn draw_waking(actions: &Vec<Action>, plot: &mut Plot, y_axis: &HashMap<u32, u32>, webgl: bool) {
    let orig = actions.first().unwrap().timestamp;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u32> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    
    for action in actions {
        if let Events::SchedWaking { base, target_cpu } = &action.event {
            xs.push(action.timestamp - orig);
            ys.push(y_axis[&action.cpu]);
            labels.push(format!("Timestamp: {}<br>Command: {}<br>Waker pid: {}<br>Wakee pid: {}<br>Target cpu: {}",
                                action.timestamp, action.process, action.pid, base.pid, target_cpu));
        }
    }

    let trace = Scatter::new(
        xs, ys)
        .mode(Mode::Markers)
        .marker(Marker::new().color(NamedColor::DarkOliveGreen).symbol(MarkerSymbol::LineNSOpen))
        .name("waking")
        .web_gl_mode(webgl)
        .hover_text_array(labels);
    plot.add_trace(trace);
}

fn draw_migrate_marks(start_time: f64, action: &Action, plot: &mut Plot, legend_group: &str, color: NamedColor, y_axis: &HashMap<u32, u32>, webgl: bool) {
    if let Events::SchedMigrateTask { base, orig_cpu, dest_cpu, state: _ } = &action.event {

        let trace = Scatter::new(
            vec![action.timestamp - start_time; 2], vec![y_axis[orig_cpu], y_axis[dest_cpu]])
            .mode(Mode::Lines)
            .line(Line::new().color(color).width(1.0))
            .hover_info(HoverInfo::None)
            .legend_group(legend_group)
            .web_gl_mode(webgl)
            .show_legend(false);
        plot.add_trace(trace);

        let hover_text = format!("Timestamp: {}<br>Command: {}<br>Pid: {}<br>Src: {}<br>Dest: {}",
                                    action.timestamp, base.command, base.pid, orig_cpu, dest_cpu);

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
        plot.add_trace(trace);
    }
}

fn draw_migrate_events(start_time: f64, action: &Action, states: &HashMap<u32, Wstate>, plot: &mut Plot, y_axis: &HashMap<u32, u32>, machine: &Machine, webgl: bool) {
    if let Events::SchedMigrateTask { base, orig_cpu, dest_cpu, state: _ } = &action.event {
        let legend_group: &str;
        let color: NamedColor;
        let (src, _) = get_socket_order(*orig_cpu, machine);
        let (dest, _) = get_socket_order(*dest_cpu, machine);

        if states.contains_key(&base.pid) {
            match states[&base.pid] {
                Wstate::Waking(..) => {
                    if src == dest {
                        legend_group = "on-socket unblock placement";
                        color = NamedColor::DeepPink;
                    } 
                    else {
                        legend_group = "off-socket unblock placement";
                        color = NamedColor::SkyBlue;
                    }
                },
                Wstate::Woken => {
                    if src == dest {
                        legend_group = "on-socket load balancing";
                        color = NamedColor::Gold;
                    }
                    else {
                        legend_group = "off-socket load balancing";
                        color = NamedColor::Orange;
                    }
                }
                Wstate::Numa(..) => {
                    legend_group = "numa balancing";
                    color = NamedColor::SeaGreen;
                }
            }
            draw_migrate_marks(start_time, action, plot, legend_group, color, y_axis, webgl);
        } else {
            dbg!(action);
        }
    }
}

fn draw_process_fork(actions: &Vec<Action>, plot: &mut Plot, y_axis: &HashMap<u32, u32>, webgl: bool) {
    let orig = actions.first().unwrap().timestamp;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u32> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    
    for action in actions {
        if let Events::SchedProcessFork { command, pid, child_command, child_pid } = &action.event {
            xs.push(action.timestamp - orig);
            ys.push(y_axis[&action.cpu]);
            labels.push(format!("Timestamp: {}<br>Command: {}<br>Pid: {}<br>Child command: {}<br>Child pid: {}",
                            action.timestamp, command, pid, child_command, child_pid));
        }
    }

    let trace = Scatter::new(
        xs, ys)
        .mode(Mode::Markers)
        .marker(Marker::new().color(NamedColor::Pink).symbol(MarkerSymbol::LineNSOpen))
        .name("process fork")
        .web_gl_mode(webgl)
        .hover_text_array(labels);
    plot.add_trace(trace);
}

fn draw_legends(plot: &mut Plot, webgl: bool) {
    plot.add_trace(Scatter::new(vec![0, 0], vec![-1, -1])
    .mode(Mode::Markers)
    .marker(Marker::new().symbol(MarkerSymbol::LineEWOpen))
    .legend_group("switch")
    .hover_info(HoverInfo::Skip)
    .web_gl_mode(webgl)
    .name("switch"));

    plot.add_trace(Scatter::new(vec![0, 0], vec![-1, -1])
    .mode(Mode::LinesMarkers)
    .marker(Marker::new().color(NamedColor::DeepPink).symbol(MarkerSymbol::TriangleRight)
            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)))
    .legend_group("on-socket unblock placement")
    .hover_info(HoverInfo::Skip)
    .web_gl_mode(webgl)
    .name("on-socket<br>unblock placement"));

    plot.add_trace(Scatter::new(vec![0, 0], vec![-1, -1])
    .mode(Mode::LinesMarkers)
    .marker(Marker::new().color(NamedColor::SkyBlue).symbol(MarkerSymbol::TriangleRight)
            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)))
    .legend_group("off-socket unblock placement")
    .hover_info(HoverInfo::Skip)
    .web_gl_mode(webgl)
    .name("off-socket<br>unblock placement"));

    plot.add_trace(Scatter::new(vec![0, 0], vec![-1, -1])
    .mode(Mode::LinesMarkers)
    .marker(Marker::new().color(NamedColor::SeaGreen).symbol(MarkerSymbol::TriangleRight)
            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)))
    .legend_group("numa balancing")
    .hover_info(HoverInfo::Skip)
    .web_gl_mode(webgl)
    .name("numa balancing"));

    plot.add_trace(Scatter::new(vec![0, 0], vec![-1, -1])
    .mode(Mode::LinesMarkers)
    .marker(Marker::new().color(NamedColor::Gold).symbol(MarkerSymbol::TriangleRight)
            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)))
    .legend_group("on-socket load balancing")
    .hover_info(HoverInfo::Skip)
    .web_gl_mode(webgl)
    .name("on-socket<br>load balancing"));

    plot.add_trace(Scatter::new(vec![0, 0], vec![-1, -1])
    .mode(Mode::LinesMarkers)
    .marker(Marker::new().color(NamedColor::Orange).symbol(MarkerSymbol::TriangleRight)
            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)))
    .legend_group("off-socket load balancing")
    .hover_info(HoverInfo::Skip)
    .web_gl_mode(webgl)
    .name("off-socket<br>load balancing"));
}

fn draw_events(actions: &Vec<Action>, plot: &mut Plot, y_axis: &HashMap<u32, u32>, webgl: bool) {
    draw_wakeup(&actions, plot, y_axis, webgl);
    draw_wakeup_new(&actions, plot, y_axis, webgl);
    draw_wakeup_no_ipi(&actions, plot, y_axis, webgl);
    draw_waking(&actions, plot, y_axis, webgl);
    draw_process_fork(&actions, plot, y_axis, webgl);
    draw_legends(plot, webgl);
}

pub fn data_graph(filepath: &str, config: &Config) {
    let graph_options = &config.graph;
    let filename = filepath.split("/").last().unwrap();

    let mut reader = TraceParser::new(filepath);
    let mut actions: Vec<Action> = Vec::new();
    while let Some((action, ..)) = reader.next_action() {
        actions.push(action);
    }
    let cpu_count = reader.cpu_count;

    let start = actions.first().unwrap().timestamp;
    let end = actions.last().unwrap().timestamp;
    let duration = end - start;

    let color_table = match graph_options.color_by.as_str() {
        "pid" => color_by_pid(&actions),
        "command" => color_by_command(&actions),
        "parent" => color_by_parent(&actions),
        _ => { panic!("Invalid color option"); }
    };

    let data = get_sched_switch_events(&actions);

    let y_axis = get_y_axis(&config.machine);

    let layout = Layout::new()
                                .title(Title::new(format!("Data Graph: {}", filename).as_str()))
                                .x_axis(
                                    Axis::new()
                                    .title(Title::new("Duration (seconds)"))
                                    .range(vec![0.0, duration])
                                    .show_grid(false))
                                .y_axis(
                                    Axis::new()
                                    .title(Title::new("Cores"))
                                    .range(vec![0, cpu_count - 1])
                                    .show_grid(false))
                                .auto_size(true);

    let mut plot = Plot::new();
    plot.set_configuration(Configuration::display_logo(plot.configuration().clone(), false));
    plot.set_configuration(Configuration::fill_frame(plot.configuration().clone(), true));

    draw_sched_switch(start, data, color_table, &mut plot, &y_axis, graph_options.webgl);
    draw_events(&actions, &mut plot, &y_axis, graph_options.webgl);

    let mut reader = TraceParser::new(filepath);
    while let Some((action, states)) = reader.next_action() {
        if let Events::SchedMigrateTask { .. } = action.event {
            let orig = actions.first().unwrap().timestamp;
            draw_migrate_events(orig, &action, states, &mut plot, &y_axis, &config.machine, graph_options.webgl);
        }
    }

    plot.set_layout(layout);
    if graph_options.launch_default_browser {
        plot.show();
    }

    plot.use_local_plotly();
    plot.write_html(format!("{}{}.html", graph_options.output_path, filename));
    if graph_options.gen_static {
        let image_format = match graph_options.filetype.as_str() {
            "png" => ImageFormat::PNG,
            "svg" => ImageFormat::SVG,
            "jpeg" => ImageFormat::JPEG,
            "webp" => ImageFormat::WEBP,
            "pdf" => ImageFormat::PDF,
            "eps" => ImageFormat::EPS,
            _ => { panic!("Invalid static file format"); }
        };
        plot.write_image(format!("{}{}.{}", graph_options.output_path, filename, graph_options.filetype), image_format, graph_options.static_res_width, graph_options.static_res_height, 1.0);
    }
}
