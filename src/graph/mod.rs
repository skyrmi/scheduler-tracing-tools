pub mod parser;
use rand::Rng;
use std::collections::HashMap;
use crate::parser::*;
use plotly::common::{ Line, Marker, Mode, Title, MarkerSymbol, HoverInfo};
use plotly::layout::{ Axis, Layout };
use plotly::{Scatter, Plot, ImageFormat, Configuration};
use plotly::color::{Rgb, NamedColor};


fn random_color() -> Rgb {
    Rgb::new(rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255))
}


fn color_by_pid(actions: &Vec<Action>) -> HashMap<u32, Rgb> {
    let mut pid_color: HashMap<u32, Rgb> = HashMap::new();
    for action in actions {
        if let Events::SchedSwitch { old_base, state: _, new_base } = &action.event {
            if let None = pid_color.get(&old_base.pid) {
                pid_color.insert(old_base.pid, random_color());
            }
            if let None = pid_color.get(&new_base.pid) {
                pid_color.insert(new_base.pid, random_color());

            }
        }
    }
    pid_color.insert(0, Rgb::new(255, 255, 255));
    pid_color
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

fn draw_sched_switch(orig: f64, data: HashMap<u32, Vec<&Action>>, pid_color: HashMap<u32, Rgb>, plot: &mut Plot) {
    for (core, switch_events) in data {
        for item in switch_events.windows(2) {
            if let Events::SchedSwitch { old_base, state, new_base} = &item[1].event {
                if old_base.pid == 0 { continue; }
                let action = item[1];
                
                let hover_text = format!("Timestamp: {}<br>From: {}<br>Pid: {}<br>State: {}<br>To: {}<br>Pid: {}",
                                            action.timestamp, old_base.command, old_base.pid, state, new_base.command, new_base.pid);

                let trace = Scatter::new(vec![item[0].timestamp - orig, item[1].timestamp - orig], vec![core, core])
                    .mode(Mode::LinesMarkers)
                    .line(Line::new().color(pid_color[&old_base.pid]).width(1.0))
                    .marker(Marker::new().symbol(MarkerSymbol::LineNSOpen))
                    .hover_text(hover_text)   
                    .name("switch")
                    .legend_group("switch")
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
    .name("switch"));
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
        if let Events::SchedWakeupNew { base, cpu } = &action.event {
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

fn draw_migrate(actions: &Vec<Action>, plot: &mut Plot) {
    let orig = actions.first().unwrap().timestamp;
    
    for action in actions {
        if let Events::SchedMigrateTask { base, orig_cpu, dest_cpu } = &action.event {

            let trace = Scatter::new(
                vec![action.timestamp - orig; 2], vec![*orig_cpu, *dest_cpu])
                .mode(Mode::Lines)
                .line(Line::new().color(NamedColor::Cyan).width(1.0))
                .hover_info(HoverInfo::None)
                .legend_group("migrate task")
                .show_legend(false);
            plot.add_trace(trace);

            let hover_text = format!("Timestamp: {}<br>Command: {}<br>Pid: {}<br>Src: {}<br>Dest: {}",
                                        action.timestamp, base.command, base.pid, orig_cpu, dest_cpu);

            let mut trace = Scatter::new(
                vec![action.timestamp - orig], vec![*dest_cpu])
                .mode(Mode::Markers)
                .name("migrate task")
                .legend_group("migrate task")
                .hover_text(hover_text)
                .show_legend(false);
            if orig_cpu < dest_cpu {
                trace = trace.marker(Marker::new().color(NamedColor::Cyan).symbol(MarkerSymbol::TriangleUp)
                            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)));
            } else {
                trace = trace.marker(Marker::new().color(NamedColor::Cyan).symbol(MarkerSymbol::TriangleDown)
                .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)));
            }
            plot.add_trace(trace);
        }
    }
    plot.add_trace(Scatter::new(vec![0, 0], vec![-1, -1])
    .mode(Mode::LinesMarkers)
    .marker(Marker::new().color(NamedColor::Cyan).symbol(MarkerSymbol::TriangleRight)
            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)))
    .legend_group("migrate task")
    .hover_info(HoverInfo::Skip)
    .name("migrate task"));
    
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

fn draw_numa_swap(actions: &Vec<Action>, plot: &mut Plot) {
    let orig = actions.first().unwrap().timestamp;

    for action in actions {
        if let Events::SchedSwapNuma { src, dest } = &action.event {
            let trace = Scatter::new(
                vec![action.timestamp - orig; 2], vec![src.cpu, dest.cpu])
                .mode(Mode::Lines)
                .line(Line::new().color(NamedColor::Goldenrod).width(1.0))
                .hover_info(HoverInfo::None)
                .legend_group("numa balancing")
                .show_legend(false);
            plot.add_trace(trace);

            let hover_text = format!("Timestamp: {}<br>Command: {}<br>Src pid: {}<br>Src nid: {}<br>Dest pid: {}<br>Dest nid: {}<br>Dest cpu: {}",
                                        action.timestamp, action.process, src.pid, src.nid, dest.pid, dest.nid, dest.cpu);

            let mut trace = Scatter::new(
                vec![action.timestamp - orig], vec![src.cpu])
                .mode(Mode::Markers)
                .name("numa swap")
                .legend_group("numa balancing")
                .hover_text(hover_text)
                .show_legend(false);
            if src.cpu > dest.cpu {
                trace = trace.marker(Marker::new().color(NamedColor::Goldenrod).symbol(MarkerSymbol::TriangleUp)
                            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)));
            } else {
                trace = trace.marker(Marker::new().color(NamedColor::Goldenrod).symbol(MarkerSymbol::TriangleDown)
                            .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)));
            }
            plot.add_trace(trace);

            let hover_text = format!("Timestamp: {}<br>Command: {}<br>Src pid: {}<br>Src nid: {}<br>Src cpu: {}<br>Dest pid: {}<br>Dest nid: {}",
                                        action.timestamp, action.process, src.pid, src.nid, src.cpu, dest.pid, dest.nid);

            let mut trace = Scatter::new(
                vec![action.timestamp - orig], vec![dest.cpu])
                .mode(Mode::Markers)
                .name("numa swap")
                .legend_group("numa balancing")
                .hover_text(hover_text)
                .show_legend(false);
            if src.cpu < dest.cpu {
                trace = trace.marker(Marker::new().color(NamedColor::Goldenrod).symbol(MarkerSymbol::TriangleUp)
                                .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)));
            } else {
                trace = trace.marker(Marker::new().color(NamedColor::Goldenrod).symbol(MarkerSymbol::TriangleDown)
                                .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)));
            }
            plot.add_trace(trace);
        }
    }
    plot.add_trace(Scatter::new(vec![0, 0], vec![-1, -1])
    .mode(Mode::LinesMarkers)
    .marker(Marker::new().color(NamedColor::Goldenrod).symbol(MarkerSymbol::TriangleRight)
                .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)))
    .legend_group("numa balancing")
    .hover_info(HoverInfo::Skip)
    .name("numa event"));
}

fn draw_numa_move(actions: &Vec<Action>, plot: &mut Plot) {
    let orig = actions.first().unwrap().timestamp;

    for action in actions {
        if let Events::SchedMoveNuma { src, dest } = &action.event {
            let trace = Scatter::new(
                vec![action.timestamp - orig; 2], vec![src.cpu, dest.cpu])
                .mode(Mode::Lines)
                .line(Line::new().color(NamedColor::Goldenrod).width(1.0))
                .hover_info(HoverInfo::None)
                .legend_group("numa balancing")
                .show_legend(false);
            plot.add_trace(trace);

            let hover_text = format!("Timestamp: {}<br>Command: {}<br>Src pid: {}<br>Src nid: {}<br>Dest nid: {}<br>Dest cpu: {}",
                                        action.timestamp, action.process, src.pid, src.nid, dest.nid, dest.cpu);

            let mut trace = Scatter::new(
                vec![action.timestamp - orig], vec![src.cpu])
                .mode(Mode::Markers)
                .name("numa move")
                .legend_group("numa balancing")
                .hover_text(hover_text)
                .show_legend(false);
            if src.cpu > dest.cpu {
                trace = trace.marker(Marker::new().color(NamedColor::Goldenrod).symbol(MarkerSymbol::TriangleUp)
                                .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)));
            } else {
                trace = trace.marker(Marker::new().color(NamedColor::Goldenrod).symbol(MarkerSymbol::TriangleDown)
                                .line(Line::new().width(1.0).color(NamedColor::DarkSlateGrey)));
            }
            plot.add_trace(trace);
        }
    }
}

fn draw_events(actions: &Vec<Action>, plot: &mut Plot) {
    draw_wakeup(&actions, plot);
    draw_wakeup_new(&actions, plot);
    draw_wakeup_no_ipi(&actions, plot);
    draw_waking(&actions, plot);
    draw_process_fork(&actions, plot);
    draw_migrate(&actions, plot);
    draw_numa_swap(&actions, plot);
    draw_numa_move(&actions, plot);
}

pub fn data_graph(filepath: &str, show: bool) {
    let (cpu_count, actions) = parse_file(filepath);
    let start = actions.first().unwrap().timestamp;
    let end = actions.last().unwrap().timestamp;
    let duration = end - start;

    let pid_color = color_by_pid(&actions);

    let data = get_sched_switch_events(&actions);


    let layout = Layout::new()
                                .title(Title::new("Data Graph"))
                                .x_axis(
                                    Axis::new()
                                    .title(Title::new("Duration (seconds)"))
                                    .range(vec![0.0, duration]).show_grid(false))
                                .y_axis(
                                    Axis::new()
                                    .title(Title::new("Cores"))
                                    .range(vec![0, cpu_count - 1]))
                                .auto_size(true);

    let mut plot = Plot::new();
    plot.set_configuration(Configuration::display_logo(plot.configuration().clone(), false));
    plot.set_configuration(Configuration::fill_frame(plot.configuration().clone(), true));

    draw_sched_switch(start, data, pid_color, &mut plot);
    draw_events(&actions, &mut plot);

    plot.set_layout(layout);
    if show {
        plot.show();
    }
    // plot.use_local_plotly();
    plot.write_html("./output/DataGraph.html");
    plot.write_image("./output/graph.pdf", ImageFormat::PDF, 1920, 1080, 1.0);
}