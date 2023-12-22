pub mod parser;
use rand::Rng;
use std::collections::HashMap;
use crate::parser::*;
use crate::read_config::Config;
use plotly::common::{ Line, Marker, Mode, Title, MarkerSymbol, HoverInfo};
use plotly::layout::{ Axis, Layout};
use plotly::{Scatter, Plot, ImageFormat, Configuration};
use plotly::color::{Rgb, NamedColor};

enum ColorTable {
    Command(HashMap<String, Rgb>),
    // Parent(HashMap<String, Rgb>),
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

fn draw_sched_switch(orig: f64, data: HashMap<u32, Vec<&Action>>, color_table: ColorTable, plot: &mut Plot) {
    for (core, switch_events) in data {
        for item in switch_events.windows(2) {
            if let Events::SchedSwitch { old_base, state, new_base} = &item[1].event {
                if old_base.pid == 0 { continue; }
                let action = item[1];
                
                let hover_text = format!("Timestamp: {}<br>From: {}<br>Pid: {}<br>State: {}<br>To: {}<br>Pid: {}",
                                            action.timestamp, old_base.command, old_base.pid, state, new_base.command, new_base.pid);

                let mut trace = Scatter::new(vec![item[0].timestamp - orig, item[1].timestamp - orig], vec![core, core])
                    .mode(Mode::LinesMarkers)
                    .marker(Marker::new().symbol(MarkerSymbol::LineNSOpen))
                    .hover_text(hover_text)   
                    .name("switch")
                    .legend_group("switch")
                    .show_legend(false);

                match &color_table {
                    ColorTable::Pid(colors) => {
                        trace = trace.line(Line::new().color(colors[&old_base.pid]).width(1.0));
                    }
                    ColorTable::Command(colors) => {
                        trace = trace.line(Line::new().color(colors[&old_base.command]).width(1.0));
                    }
                    // _ => {}
                }

                plot.add_trace(trace);
            }
        }
    }
}   

fn draw_wakeup(actions: &Vec<Action>, plot: &mut Plot) {
    let orig = actions.first().unwrap().timestamp;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u32> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    
    for action in actions {
        if let Events::SchedWakeup { base, .. } = &action.event {
            xs.push(action.timestamp - orig);
            ys.push(action.cpu);
            labels.push(format!("Timestamp: {}<br>Waker: {}<br>Waker pid: {}<br>Wakee: {}<br>Wakee pid: {}",
                            action.timestamp, action.process, action.pid, base.command, base.pid));
        }
    }

    let trace = Scatter::new(
        xs, ys)
        .mode(Mode::Markers)
        .marker(Marker::new().color(NamedColor::RoyalBlue).symbol(MarkerSymbol::LineNSOpen))
        .name("wakeup")
        .hover_text_array(labels);
    plot.add_trace(trace);
}

fn draw_wakeup_new(actions: &Vec<Action>, plot: &mut Plot) {
    let orig = actions.first().unwrap().timestamp;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u32> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    
    for action in actions {
        if let Events::SchedWakeupNew { base, parent_cpu: _ , cpu } = &action.event {
            xs.push(action.timestamp - orig);
            ys.push(action.cpu);
            labels.push(format!("Timestamp: {}<br>Command: {}<br>Waker pid: {}<br>Wakee pid: {}Target cpu: {}",
                            action.timestamp, action.process, action.pid, base.pid, cpu));
        }
    }

    let trace = Scatter::new(
        xs, ys)
        .mode(Mode::Markers)
        .marker(Marker::new().color(NamedColor::Brown).symbol(MarkerSymbol::LineNSOpen))
        .name("wakeup new")
        .hover_text_array(labels);
    plot.add_trace(trace);
}

fn draw_wakeup_no_ipi(actions: &Vec<Action>, plot: &mut Plot) {
    let orig = actions.first().unwrap().timestamp;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u32> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    
    for action in actions {
        if let Events::SchedWakeIdleNoIpi { .. } = &action.event {
            xs.push(action.timestamp - orig);
            ys.push(action.cpu);
            labels.push(format!("Timestamp: {}<br>Command: {}<br>Pid: {}", action.timestamp, action.process, action.pid));
        }
    }

    let trace = Scatter::new(
        xs, ys)
        .mode(Mode::Markers)
        .marker(Marker::new().color(NamedColor::LimeGreen).symbol(MarkerSymbol::LineNSOpen))
        .name("wake idle without ipi")
        .hover_text_array(labels);
    plot.add_trace(trace);
}

fn draw_waking(actions: &Vec<Action>, plot: &mut Plot) {
    let orig = actions.first().unwrap().timestamp;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u32> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    
    for action in actions {
        if let Events::SchedWaking { base, target_cpu } = &action.event {
            xs.push(action.timestamp - orig);
            ys.push(action.cpu);
            labels.push(format!("Timestamp: {}<br>Command: {}<br>Waker pid: {}<br>Wakee pid: {}<br>Target cpu: {}",
                                action.timestamp, action.process, action.pid, base.pid, target_cpu));
        }
    }

    let trace = Scatter::new(
        xs, ys)
        .mode(Mode::Markers)
        .marker(Marker::new().color(NamedColor::DarkOliveGreen).symbol(MarkerSymbol::LineNSOpen))
        .name("waking")
        .hover_text_array(labels);
    plot.add_trace(trace);
}

fn draw_migrate_marks(start_time: f64, action: &Action, plot: &mut Plot, legend_group: &str, color: NamedColor) {
    if let Events::SchedMigrateTask { base, orig_cpu, dest_cpu, state: _ } = &action.event {

        let trace = Scatter::new(
            vec![action.timestamp - start_time; 2], vec![*orig_cpu, *dest_cpu])
            .mode(Mode::Lines)
            .line(Line::new().color(color).width(1.0))
            .hover_info(HoverInfo::None)
            .legend_group(legend_group)
            .show_legend(false);
        plot.add_trace(trace);

        let hover_text = format!("Timestamp: {}<br>Command: {}<br>Pid: {}<br>Src: {}<br>Dest: {}",
                                    action.timestamp, base.command, base.pid, orig_cpu, dest_cpu);

        let mut trace = Scatter::new(
            vec![action.timestamp - start_time], vec![*dest_cpu])
            .mode(Mode::Markers)
            .name(legend_group)
            .legend_group(legend_group)
            .hover_text(hover_text)
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

fn draw_migrate_events(start_time: f64, action: &Action, states: &HashMap<u32, Wstate>, plot: &mut Plot) {
    if let Events::SchedMigrateTask { base, ..} = &action.event {
        let legend_group: &str;
        let color: NamedColor;
        if states.contains_key(&base.pid) {
            match states[&base.pid] {
                Wstate::Waking(..) => { 
                    legend_group = "unblock placement";
                    color = NamedColor::DeepPink;
                },
                Wstate::Woken => {
                    legend_group = "load balancing";
                    color = NamedColor::Goldenrod;
                }
                Wstate::Numa(..) => {
                    legend_group = "numa balancing";
                    color = NamedColor:: SeaGreen;
                }
            }
            draw_migrate_marks(start_time, action, plot, legend_group, color);
        } else {
            dbg!(action);
        }
    }
}

fn draw_process_fork(actions: &Vec<Action>, plot: &mut Plot) {
    let orig = actions.first().unwrap().timestamp;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u32> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    
    for action in actions {
        if let Events::SchedProcessFork { command, pid, child_command, child_pid } = &action.event {
            xs.push(action.timestamp - orig);
            ys.push(action.cpu);
            labels.push(format!("Timestamp: {}<br>Command: {}<br>Pid: {}<br>Child command: {}<br>Child pid: {}",
                            action.timestamp, command, pid, child_command, child_pid));
        }
    }

    let trace = Scatter::new(
        xs, ys)
        .mode(Mode::Markers)
        .marker(Marker::new().color(NamedColor::Pink).symbol(MarkerSymbol::LineNSOpen))
        .name("process fork")
        .hover_text_array(labels);
    plot.add_trace(trace);
}

fn draw_legends(plot: &mut Plot) {
    plot.add_trace(Scatter::new(vec![0, 0], vec![-1, -1])
    .mode(Mode::Markers)
    .marker(Marker::new().symbol(MarkerSymbol::LineEWOpen))
    .legend_group("switch")
    .hover_info(HoverInfo::Skip)
    .name("switch"));

    plot.add_trace(Scatter::new(vec![0, 0], vec![-1, -1])
    .mode(Mode::LinesMarkers)
    .marker(Marker::new().color(NamedColor::DeepPink).symbol(MarkerSymbol::TriangleRight)
            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)))
    .legend_group("unblock placement")
    .hover_info(HoverInfo::Skip)
    .name("unblock placement"));

    plot.add_trace(Scatter::new(vec![0, 0], vec![-1, -1])
    .mode(Mode::LinesMarkers)
    .marker(Marker::new().color(NamedColor::SeaGreen).symbol(MarkerSymbol::TriangleRight)
            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)))
    .legend_group("numa balancing")
    .hover_info(HoverInfo::Skip)
    .name("numa balancing"));

    plot.add_trace(Scatter::new(vec![0, 0], vec![-1, -1])
    .mode(Mode::LinesMarkers)
    .marker(Marker::new().color(NamedColor::Goldenrod).symbol(MarkerSymbol::TriangleRight)
            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)))
    .legend_group("load balancing")
    .hover_info(HoverInfo::Skip)
    .name("load balancing"));
}

fn draw_events(actions: &Vec<Action>, plot: &mut Plot) {
    draw_wakeup(&actions, plot);
    draw_wakeup_new(&actions, plot);
    draw_wakeup_no_ipi(&actions, plot);
    draw_waking(&actions, plot);
    draw_process_fork(&actions, plot);
    draw_legends(plot);
}

pub fn data_graph(filepath: &str, config: &Config) {
    let graph_options = &config.graph;
    let filename = filepath.split("/").last().unwrap();

    let mut plot = Plot::new();

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
        _ => { panic!("Invalid color option"); }
    };

    let data = get_sched_switch_events(&actions);

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

    plot.set_configuration(Configuration::display_logo(plot.configuration().clone(), false));
    plot.set_configuration(Configuration::fill_frame(plot.configuration().clone(), true));

    draw_sched_switch(start, data, color_table, &mut plot);
    draw_events(&actions, &mut plot);

    let mut reader = TraceParser::new(filepath);
    while let Some((action, states)) = reader.next_action() {
        if let Events::SchedMigrateTask { .. } = action.event {
            let orig = actions.first().unwrap().timestamp;
            draw_migrate_events(orig, &action, states, &mut plot);
        }
    }

    plot.set_layout(layout);
    if graph_options.launch_default_browser {
        plot.show();
    }

    plot.write_html(format!("./output/{}.html", filename));
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
        plot.write_image(format!("./output/{}.{}", filename, graph_options.filetype), image_format, graph_options.static_res_width, graph_options.static_res_height, 1.0);
    }
}
