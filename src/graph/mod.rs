pub mod parser;

use rand::{Rng, random};
use std::collections::HashMap;
use crate::parser::*;
use plotly::common::{
    ColorScale, ColorScalePalette, DashType, Fill, Font, Line, LineShape, Marker, Mode, Title, Label, MarkerSymbol,
};
use plotly::layout::{
    Axis, GridPattern, Layout, LayoutGrid, Margin, Shape, ShapeLayer, ShapeLine,
    ShapeType, self, NewShape, Annotation,
};

use plotly::{Scatter};
use plotly::{Plot, ImageFormat};
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
            if let Events::SchedSwitch { old_base, ..} = &item[1].event {
                
                let trace = Scatter::new(vec![item[0].timestamp - orig, item[1].timestamp - orig], vec![core, core])
                    .mode(Mode::LinesMarkers)
                    .line(Line::new().color(pid_color[&old_base.pid]).width(1.0))
                    .marker(Marker::new().symbol(MarkerSymbol::LineNSOpen))
                    .hover_text(old_base.pid.to_string())
                    .name(old_base.command.to_string())
                    .show_legend(false);

                plot.add_trace(trace);
            }
        }
    }
}

fn draw_wakeup(actions: &Vec<Action>, plot: &mut Plot) {
    let orig = actions.first().unwrap().timestamp;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u32> = Vec::new();
    let mut timestamps: Vec<String> = Vec::new();
    
    for action in actions {
        if let Events::SchedWakeup { .. } = &action.event {
            xs.push(action.timestamp - orig);
            ys.push(action.cpu);
            timestamps.push(action.timestamp.to_string());
        }
    }

    let trace = Scatter::new(
        xs, ys)
        .mode(Mode::Markers)
        .marker(Marker::new().color(NamedColor::RoyalBlue).symbol(MarkerSymbol::LineNSOpen))
        .name("wakeup")
        .hover_text_array(timestamps);
    plot.add_trace(trace);
}

fn draw_wakeup_no_ipi(actions: &Vec<Action>, plot: &mut Plot) {
    let orig = actions.first().unwrap().timestamp;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u32> = Vec::new();
    let mut timestamps: Vec<String> = Vec::new();
    
    for action in actions {
        if let Events::SchedWakeIdleNoIpi { .. } = &action.event {
            xs.push(action.timestamp - orig);
            ys.push(action.cpu);
            timestamps.push(action.timestamp.to_string());
        }
    }

    let trace = Scatter::new(
        xs, ys)
        .mode(Mode::Markers)
        .marker(Marker::new().color(NamedColor::LimeGreen).symbol(MarkerSymbol::LineNSOpen))
        .name("wake idle without ipi")
        .hover_text_array(timestamps);
    plot.add_trace(trace);
}

fn draw_waking(actions: &Vec<Action>, plot: &mut Plot) {
    let orig = actions.first().unwrap().timestamp;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<u32> = Vec::new();
    let mut timestamps: Vec<String> = Vec::new();
    
    for action in actions {
        if let Events::SchedWaking { .. } = &action.event {
            xs.push(action.timestamp - orig);
            ys.push(action.cpu);
            timestamps.push(action.timestamp.to_string());
        }
    }

    let trace = Scatter::new(
        xs, ys)
        .mode(Mode::Markers)
        .marker(Marker::new().color(NamedColor::DarkOliveGreen).symbol(MarkerSymbol::LineNSOpen))
        .name("waking")
        .hover_text_array(timestamps);
    plot.add_trace(trace);
}



pub fn data_graph(show: bool) {
    let (cpu_count, actions) = parse_file();
    let start = actions.first().unwrap().timestamp;
    let end = actions.last().unwrap().timestamp;
    let duration = end - start;

    let pid_color = color_by_pid(&actions);

    let data = get_sched_switch_events(&actions);


    let mut layout = Layout::new()
                                .title(Title::new("Data Graph"))
                                .x_axis(Axis::new().range(vec![0.0, duration]).show_grid(false))
                                .y_axis(Axis::new().range(vec![0, cpu_count - 1]))
                                .width(1366)
                                .height(800);

    let mut plot = Plot::new();

    draw_sched_switch(start, data, pid_color, &mut plot);
    draw_wakeup(&actions, &mut plot);
    draw_wakeup_no_ipi(&actions, &mut plot);
    draw_waking(&actions, &mut plot);

    plot.set_layout(layout);
    if show {
        plot.show();
    }
    // plot.use_local_plotly();
    plot.write_html("./output/DataGraph.html");
    // plot.write_image("test.png", ImageFormat::PDF, 1920, 1080, 1.0)
}