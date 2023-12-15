pub mod parser;

use rand::Rng;
use std::iter::Iterator;
use std::collections::HashMap;
use plotters::prelude::*;
use crate::parser::*;


pub fn data_graph() {
    let (cpu_count, actions) = parse_file();
    let duration = actions.last().unwrap().timestamp - actions.first().unwrap().timestamp;

    let mut pid_color: HashMap<u32, RGBColor> = HashMap::new();
    for action in &actions {
        if let Events::SchedSwitch { old_base, state: _, new_base } = &action.event {
            if let None = pid_color.get(&old_base.pid) {
                pid_color.insert(old_base.pid, RGBColor(rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255)));
            }
            if let None = pid_color.get(&new_base.pid) {
                pid_color.insert(new_base.pid, RGBColor(rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255)));

            }
        }
    }
    pid_color.insert(0, WHITE);
    // dbg!(&pid_color);

    let mut data: HashMap<u32, Vec<&Action>> = HashMap::new();
    for action in &actions {
        if let Events::SchedSwitch { .. } = &action.event {
            let entry = data.entry(action.cpu).or_insert_with(Vec::new);
            entry.push(action);
        }
    }
    // dbg!(&data);


    let root_area = SVGBackend::new("./output/image.svg", (1920, 1080)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();

    let mut ctx = ChartBuilder::on(&root_area)
        .set_label_area_size(LabelAreaPosition::Left, 40)
        .set_label_area_size(LabelAreaPosition::Bottom, 40)
        .caption("Data graph", ("sans-serif", 25))
        .build_cartesian_2d(0.0..duration + 0.5, 0.0..cpu_count as f64 - 1.0)
        .unwrap();

    ctx.configure_mesh().x_desc("Duration: seconds").y_desc("Cpu core count").disable_mesh().draw().unwrap();

    for (core, switch_events) in data {
        let mut ranges: Vec<(f64, f64, RGBColor)> = Vec::new();
        let orig = actions.first().unwrap().timestamp;
        for item in switch_events.windows(2) {
            if let Events::SchedSwitch { old_base, ..} = &item[1].event {
                let pid = old_base.pid;
                
                // let mut min: f64 = 0.0;
                // if item[1].timestamp - item[0].timestamp < 0.001 {
                //     println!("{}", item[1].timestamp - item[0].timestamp);
                //     min += 0.1;
                // }
                // println!("{min}");
                ranges.push((item[0].timestamp - orig, item[1].timestamp - orig, pid_color[&pid]));
                
                // ctx.draw_series(LineSeries::new(
                //     vec![(item[0].timestamp - orig, core), (item[1].timestamp - orig, core)],
                //      pid_color[&pid])).unwrap();
                
            }
        }
        
        ctx.draw_series(ranges.iter().map(|y| {
            let mut bar = Rectangle::new([
                (y.0, core as f64 - 0.25), 
                (y.1, core as f64 + 0.25),
            ], y.2.filled());
            bar.set_margin(0, 0, 0, 0);
            bar
        }))
        .unwrap();
    }

}