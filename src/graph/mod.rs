pub mod parser;
use rand::Rng;
use std::collections::HashMap;
use crate::parser::*;
use crate::read_config::{ Config, Machine, Graph };
use plotly::common::{ Line, Marker, Mode, Title, MarkerSymbol, HoverInfo};
use plotly::layout::{ Axis, Layout };
use plotly::{ Scatter, Plot, ImageFormat, Configuration, Trace };
use plotly::color::{ Rgb, NamedColor };

// Scatter object to store notch-only events
// Drawing all such events at once is more efficient than adding their trace individually
struct ScatterObject {
    xs: Vec<f64>,
    ys: Vec<u32>,
    mode: Mode,
    name: String,
    color: NamedColor,
    color_array: Vec<Rgb>,
    hover_text: Vec<String>,
}

impl ScatterObject {
    fn new(mode: Mode, name: &str, color: NamedColor) -> Self {
        ScatterObject {
            xs: Vec::new(),
            ys: Vec::new(),
            mode,
            name: name.to_string(),
            color,
            color_array: Vec::new(),
            hover_text: Vec::new(),
        }
    }
}

// constructs a Hashmap for events containing only a notch
// Adding events: insert an event and its color here
//      followed by adding its match condition in draw_traces()
fn marker_events_object() -> HashMap<String, ScatterObject> {
    let mut map: HashMap<String, ScatterObject> = HashMap::new();
    let events = vec![("wakeup", NamedColor::RoyalBlue),
                            ("wakeup new", NamedColor::Brown),
                            ("wake idle no ipi", NamedColor::LimeGreen),
                            ("waking", NamedColor::DarkOliveGreen),
                            ("process fork", NamedColor::Pink)];

    for (name, color) in events {
        map.insert(name.to_string(), ScatterObject::new(Mode::Markers, name, color));
    }
    map
}

fn get_frequency_map() -> HashMap<String, u32> {
    let events = vec![
        "wakeup",
        "wakeup new",
        "wake idle no ipi",
        "waking",
        "process fork",
        "on-socket<br>unblock placement",
        "off-socket<br>unblock placement",
        "on-socket<br>load balancing",
        "off-socket<br>load balancing",
        "numa balancing"
    ];
    let mut frequency: HashMap<String, u32> = HashMap::new();
    for event in events {
        frequency.insert(event.to_string(), 0);
    }
    frequency
}

fn set_marker_size(cpu_count: u32) -> usize{
    let marker_size: usize;
    if cpu_count <= 64 {
        marker_size = 6;
    }
    else if cpu_count <= 128 {
        marker_size = 5
    } 
    else {
        marker_size = 3;
    }
    marker_size
}


// Get a random color
fn random_color() -> Rgb {
    Rgb::new(rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255))
}


// Different coloring options for sched_switch events
enum ColorTable {
    Command(HashMap<String, Rgb>),
    Parent(HashMap<u32, Rgb>),
    Pid(HashMap<u32, Rgb>),
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


// find the socket for the given cpu
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

// If socket_order = true, transform the y-axis to have cpus in the same socket together
// Can then be used for the y-value of any point
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


// group the switch events by cpu, order is the same as the input vector
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

fn draw_switch_markers(plot: &mut Plot, switch_markers: ScatterObject, options: &Graph, marker_size: usize) {
    if options.events.show_events || options.events.show_switch {
        // draw the switch event notches
        plot.add_trace(Scatter::new(
            switch_markers.xs, switch_markers.ys)
            .mode(Mode::Markers)
            .marker(Marker::new().symbol(MarkerSymbol::LineNSOpen).color_array(switch_markers.color_array).size(marker_size))
            .name(&switch_markers.name)
            .hover_text_array(switch_markers.hover_text)
            .legend_group(switch_markers.name)
            .show_legend(false)
            .web_gl_mode(options.webgl)
        );

        // draw the legend for the switch events
        plot.add_trace(Scatter::new(vec![0], vec![-1])
            .mode(Mode::Markers)
            .marker(Marker::new().symbol(MarkerSymbol::LineEWOpen))
            .legend_group("switch")
            .hover_info(HoverInfo::Skip)
            .name("switch"));
    }
}


fn draw_sched_switch(orig: f64, data: HashMap<u32, Vec<&Action>>, color_table: ColorTable, plot: &mut Plot, switch_markers: &mut ScatterObject, y_axis: &HashMap<u32, u32>, options: &Graph, marker_size: usize) {
    let mut transparent_markers = ScatterObject::new(Mode::Markers, "switch", NamedColor::White);
    for (core, switch_events) in data {
        for item in switch_events.windows(2) {
            if let Events::SchedSwitch { old_command, old_pid, state, new_command, new_pid } = &item[1].event {
                if *old_pid == 0 { continue; }
                if !options.interactive && item[1].timestamp - item[0].timestamp < options.limit {
                    continue;
                }
                
                let hover_text = format!("Timestamp: {}<br>From: {}<br>Pid: {}<br>State: {}<br>To: {}<br>Pid: {}",
                                            item[1].timestamp, old_command, old_pid, state, new_command, new_pid);
                

                // draw the switch event lines
                let mut trace = Scatter::new(vec![item[0].timestamp - orig, item[1].timestamp - orig], vec![y_axis[&core], y_axis[&core]])
                                                            .mode(Mode::Lines)
                                                            .hover_info(HoverInfo::Skip)   
                                                            .web_gl_mode(options.webgl)
                                                            .show_legend(false);
                
                
                let color = match &color_table {
                    ColorTable::Pid(colors) => colors[old_pid],
                    ColorTable::Command(colors) => colors[old_command],
                    ColorTable::Parent(colors) => colors[old_pid]
                };
                trace = trace.line(Line::new().color(color).width(1.0));
                plot.add_trace(trace);

                // store the switch event notches in a scatterobject to draw together
                switch_markers.xs.push(item[1].timestamp - orig);
                switch_markers.ys.push(y_axis[&core]);
                switch_markers.hover_text.push(hover_text);
                switch_markers.color_array.push(color);

                // transparent markers: workaround for showing hover text on lines
                let hover_text = format!("Command: {}<br>Pid: {}", old_command, old_pid);
                for i in 1..options.line_marker_count {
                    transparent_markers.xs.push(item[0].timestamp - orig + (item[1].timestamp - item[0].timestamp) / options.line_marker_count as f64 * i as f64);
                    transparent_markers.ys.push(y_axis[&core]);
                    transparent_markers.color_array.push(color);
                    transparent_markers.hover_text.push(hover_text.to_string());
                }
            }
        }
    }
    // draw the transparent markers
    plot.add_trace(
        Scatter::new(transparent_markers.xs, transparent_markers.ys)
            .mode(Mode::Markers)
            .marker(Marker::new().symbol(MarkerSymbol::LineNSOpen).color_array(transparent_markers.color_array).opacity(0.0).size(marker_size))
            .hover_text_array(transparent_markers.hover_text)
            .legend_group("switch")
            .hover_info(HoverInfo::Text)
            .show_legend(false)
            .web_gl_mode(true)
    )
}   

fn draw_migrate_marks(start_time: f64, action: &Action, traces: &mut Vec<Box<dyn Trace>>, legend_group: &str, color: NamedColor, y_axis: &HashMap<u32, u32>, webgl: bool, marker_size: usize) {
    if let Events::SchedMigrateTask { command, pid, orig_cpu, dest_cpu, .. } = &action.event {

        // draw the migrate event lines
        let trace = Scatter::new(
            vec![action.timestamp - start_time; 2], vec![Some(y_axis[orig_cpu]), Some(y_axis[dest_cpu])])
            .mode(Mode::Lines)
            .line(Line::new().color(color).width(1.0))
            .hover_info(HoverInfo::None)
            .legend_group(legend_group)
            .web_gl_mode(webgl)
            .show_legend(false);
        traces.push(trace);

        let hover_text = format!("Timestamp: {}<br>Command: {}<br>Pid: {}<br>Src: {}<br>Dest: {}",
                                    action.timestamp, command, pid, orig_cpu, dest_cpu);

        // draw the migrate event notches
        // possible performance improvement by using a ScatterObject instead of drawing here
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
                        .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)).size(marker_size));
        } else {
            trace = trace.marker(Marker::new().color(color).symbol(MarkerSymbol::TriangleDown)
            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)).size(marker_size));
        }
        traces.push(trace);
    }
}


// Determine type of migrate event and draw
fn classify_migrate_event(start_time: f64, action: &Action, states: &HashMap<u32, Wstate>, traces: &mut Vec<Box<dyn Trace>>, y_axis: &HashMap<u32, u32>, config: &Config, frequency: &mut HashMap<String, u32>, marker_size: usize) {
    if let Events::SchedMigrateTask { command: _, pid, orig_cpu, dest_cpu, state: _ } = &action.event {
        let legend_group: &str;
        let color: NamedColor;
        let (src, _) = get_socket_order(*orig_cpu, &config.machine);
        let (dest, _) = get_socket_order(*dest_cpu, &config.machine);

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
            draw_migrate_marks(start_time, action, traces, legend_group, color, y_axis, config.graph.webgl, marker_size);
            frequency.insert(legend_group.to_string(), frequency[legend_group] + 1);
        }
    }
}

fn draw_legends(plot: &mut Plot, frequency: HashMap<String, u32>, options: &Graph) {
    let marker_legends = vec![("wakeup", NamedColor::RoyalBlue),
                                            ("wakeup new", NamedColor::Brown),
                                            ("wake idle no ipi", NamedColor::LimeGreen),
                                            ("waking", NamedColor::DarkOliveGreen),
                                            ("process fork", NamedColor::Pink)];

    let migrate_legends = vec![
                                    ("on-socket<br>unblock placement", NamedColor::DeepPink),
                                    ("off-socket<br>unblock placement", NamedColor::SkyBlue),
                                    ("numa balancing", NamedColor::SeaGreen),
                                    ("on-socket<br>load balancing", NamedColor::Gold),
                                    ("off-socket<br>load balancing", NamedColor::Orange)];

    // marker legends: containing only a notch 
    if options.events.show_events || options.events.show_marker_only {
        for (legend_group, color) in marker_legends {
            if frequency.contains_key(legend_group) {
                let name = format!("{} ({})", legend_group, frequency[legend_group]);
                plot.add_trace(Scatter::new(vec![0], vec![-1])
                .mode(Mode::LinesMarkers)
                .marker(Marker::new().color(color).symbol(MarkerSymbol::LineNSOpen))
                .line(Line::new().width(1.0))
                .legend_group(legend_group)
                .hover_info(HoverInfo::Skip)
                .name(&name));
            }
        }
    }

    // migrate events: contain both lines and notches
    if options.events.show_events || options.events.show_migrate {
        for (legend_group, color) in migrate_legends {
            if frequency.contains_key(legend_group) {
                let name = format!("{} ({})", legend_group, frequency[legend_group]);
                plot.add_trace(Scatter::new(vec![0], vec![-1])
                .mode(Mode::LinesMarkers)
                .marker(Marker::new().color(color).symbol(MarkerSymbol::TriangleRight)
                        .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)))
                .legend_group(&legend_group)
                .hover_info(HoverInfo::Skip)
                .name(name));
            }
        }
    }
}


// add event to ScatterObject
fn add_event(marker_events: &mut HashMap<String, ScatterObject>, action: &Action, start_time: f64, y_axis: &HashMap<u32, u32>, name: &str, hover_text: String) {
    if let Some(entry) = marker_events.get_mut(name) {
        entry.xs.push(action.timestamp - start_time);
        entry.ys.push(y_axis[&action.cpu]);
        entry.hover_text.push(hover_text);
    }
}

// draw the ScatterObject for marker-only events
fn draw_marker_event(plot: &mut Plot, marker_events: HashMap<String, ScatterObject>, options: &Graph, marker_size: usize) {
    for (_, event) in marker_events {
        let trace = Scatter::new(
            event.xs, event.ys)
            .mode(event.mode)
            .marker(Marker::new().color(event.color).symbol(MarkerSymbol::LineNSOpen).size(marker_size))
            .name(&event.name)
            .legend_group(event.name)
            .web_gl_mode(options.webgl)
            .hover_text_array(event.hover_text)
            .show_legend(false);
        plot.add_trace(trace);
    }
}

// find the first sleep command's exit point
// It then becomes the starting point of the plot
fn find_sleep(reader: &mut TraceParser, options: &Graph) {
    if options.sleep {
        while let Some((action, ..)) = reader.next_action() {
            if let Events::SchedProcessExit { command, .. } = &action.event {
                if command == "sleep" {
                    reader.first_timestamp = Some(action.timestamp);
                    break;
                }
            }
        }
    }
}

fn draw_traces(filepath: &str, config: &Config, plot: &mut Plot) -> TraceParser {
    let mut reader = TraceParser::new(filepath);
    let mut switch_events: Vec<Action> = Vec::new();
    let mut boundary_events: HashMap<u32, Action> =  HashMap::new();
    let mut fork_events: Vec<Action> = Vec::new();
    let mut migrate_traces: Vec<Box<dyn Trace>> = Vec::new();
    let mut marker_events = marker_events_object();
    let mut frequency: HashMap<String, u32> = get_frequency_map();

    let options = &config.graph;
    let y_axis = get_y_axis(&config.machine, options.socket_order, reader.cpu_count);
    let marker_size = set_marker_size(reader.cpu_count);

    find_sleep(&mut reader, options);

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
        
        // match and store the events
        let mut name = "";
        match &action.event {
            Events::SchedSwitch { .. } => {
                name = "switch";
                if options.custom_range && !boundary_events.is_empty()  {
                    for (_, v) in boundary_events.drain() {
                        switch_events.push(v);
                    }
                }
                switch_events.push(action);
            },
            Events::SchedWakeup { command, pid, .. } => {
                name = "wakeup";
                let hover_text = format!("Timestamp: {}<br>Waker: {}<br>Waker pid: {}<br>Wakee: {}<br>Wakee pid: {}",
                                action.timestamp, action.process, action.pid, command, pid);
                add_event(&mut marker_events, &action, start_time, &y_axis, name, hover_text);

            },
            Events::SchedWakeupNew { command: _, pid, parent_cpu: _, cpu } => {
                name = "wakeup new";
                let hover_text = format!("Timestamp: {}<br>Command: {}<br>Waker pid: {}<br>Wakee pid: {}<br>Target cpu: {}",
                                action.timestamp, action.process, action.pid, pid, cpu);
                add_event(&mut marker_events, &action, start_time, &y_axis, name, hover_text);
            },
            Events::SchedWakeIdleNoIpi { .. } => {
                name = "wake idle no ipi";
                let hover_text = format!("Timestamp: {}<br>Command: {}<br>Pid: {}", action.timestamp, action.process, action.pid);
                add_event(&mut marker_events, &action, start_time, &y_axis, name, hover_text);
            }
            Events::SchedWaking { command: _, pid, target_cpu } => {
                name = "waking";
                let hover_text = format!("Timestamp: {}<br>Command: {}<br>Waker pid: {}<br>Wakee pid: {}<br>Target cpu: {}",
                                action.timestamp, action.process, action.pid, pid, target_cpu);
                add_event(&mut marker_events, &action, start_time, &y_axis, name, hover_text);
            },
            Events::SchedProcessFork { command, pid, child_command, child_pid } => {
                name = "process fork";
                let hover_text = format!("Timestamp: {}<br>Command: {}<br>Pid: {}<br>Child command: {}<br>Child pid: {}",
                                action.timestamp, command, pid, child_command, child_pid);
                add_event(&mut marker_events, &action, start_time, &y_axis, name, hover_text);
                fork_events.push(action);
            },
            Events::SchedMigrateTask { .. } => {
                name = "migrate task";
                classify_migrate_event(start_time, &action, states, &mut migrate_traces, &y_axis, config, &mut frequency, marker_size);
            }
            _ => { }
        }
        if frequency.contains_key(name) {
            frequency.insert(name.to_string(), frequency[name] + 1);
        }
    }

    let color_table = match options.color_by.as_str() {
        "pid" => color_by_pid(&switch_events),
        "command" => color_by_command(&switch_events),
        "parent" => color_by_parent(&fork_events),
        _ => { panic!("Invalid color option"); }
    };

    // group and draw switch events
    let switch_events = get_sched_switch_events(&switch_events);
    let mut switch_markers = ScatterObject::new(Mode::LinesMarkers, "switch", NamedColor::White);
    draw_sched_switch(reader.first_timestamp.unwrap(), switch_events, color_table, plot, &mut switch_markers, &y_axis, options, marker_size);
    draw_switch_markers(plot, switch_markers, options, marker_size);

    if options.events.show_events || options.events.show_marker_only {
        draw_marker_event(plot, marker_events, options, marker_size);
    }
    if options.events.show_events || options.events.show_migrate {
        plot.add_traces(migrate_traces);
    }
    draw_legends(plot, frequency, options);
    reader
}

pub fn data_graph(filepath: &str, config: &Config) {
    let options = &config.graph;
    let filename = filepath.split("/").last().unwrap();
    let mut plot = Plot::new();

    let reader = draw_traces(filepath, config, &mut plot);
    
    let duration: Vec<f64>;
    let x_axis_title: String;
    if options.custom_range {
        duration = vec![options.min, options.max];
        x_axis_title = format!("Duration: {} seconds", options.max - options.min);
    } else {
        duration = vec![0.0, reader.last_timestamp.unwrap() - reader.first_timestamp.unwrap()];
        x_axis_title = format!("Duration: {:.6?} seconds", reader.last_timestamp.unwrap() - reader.first_timestamp.unwrap())
    }

    let mut y_axis_title = String::from("Cores"); 
    if options.socket_order {
        y_axis_title.push_str(" (socket order)")
    }

    let mut layout = Layout::new()
                            .x_axis(
                                Axis::new()
                                .title(Title::new(&x_axis_title))
                                .range(duration)
                                .show_grid(false))
                            .y_axis(
                                Axis::new()
                                .title(Title::new(&y_axis_title))
                                .range(vec![0, reader.cpu_count - 1])
                                .show_grid(false))
                            .auto_size(true);


    if options.line_marker_count > 0 && options.line_marker_count <= 25 {
        layout = layout.hover_distance(100);
    }

    if options.show_title {
        layout = layout.title(Title::new(format!("Data Graph: {}", filename).as_str()));
    }

    plot.set_configuration(Configuration::display_logo(plot.configuration().clone(), false));
    plot.set_configuration(Configuration::fill_frame(plot.configuration().clone(), true));

    if !options.interactive {
        plot.set_configuration(Configuration::static_plot(plot.configuration().clone(), true));
    }

    plot.set_layout(layout);
    plot.use_local_plotly();
    if options.show_html && options.browser == "" {
        plot.show();
    }

    if options.create_html || options.show_html {
        plot.write_html(format!("{}{}.html", options.output_path, filename));
    }

    if options.show_html && options.browser != "" {
        open::with(format!("{}{}.html", options.output_path, filename), options.browser.to_string()).expect("Could not open alternate browser");
    }

    if options.static_options.gen_static {
        let image_format = match options.static_options.filetype.as_str() {
            "png" => ImageFormat::PNG,
            "svg" => ImageFormat::SVG,
            "jpeg" => ImageFormat::JPEG,
            "webp" => ImageFormat::WEBP,
            "pdf" => ImageFormat::PDF,
            "eps" => ImageFormat::EPS,
            _ => { panic!("Invalid static file format"); }
        };
        plot.write_image(format!("{}{}.{}", options.output_path, filename, options.static_options.filetype), image_format, options.static_options.static_res_width, options.static_options.static_res_height, 1.0);
    }
}
