pub mod parser;

use rand::Rng;
use std::collections::HashMap;
use plotters::prelude::*;
use crate::parser::*;

pub fn data_graph() {
    let (cpu_count, actions) = parse_file();
    let duration = actions.last().unwrap().timestamp - actions.first().unwrap().timestamp;

    let colors = [RED, GREEN, BLUE, YELLOW, CYAN, MAGENTA];
    let mut index = 0;
    let mut pid_color: HashMap<u32, RGBColor> = HashMap::new();
    for action in &actions {
        if let Events::SchedSwitch { old_base, state, new_base } = &action.event {
            if let None = pid_color.get(&old_base.pid) {
                // pid_color.insert(old_base.pid, colors[index % 6]);
                pid_color.insert(old_base.pid, RGBColor(rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255)));
            }
            index += 1;
            if let None = pid_color.get(&new_base.pid) {
                // pid_color.insert(new_base.pid, colors[index % 6]);
                pid_color.insert(new_base.pid, RGBColor(rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255), rand::thread_rng().gen_range(0..=255)));

            }
            index += 1;
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

    let root_area = BitMapBackend::new("./image.png", (1280, 720)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();

    let mut ctx = ChartBuilder::on(&root_area)
        .set_label_area_size(LabelAreaPosition::Left, 40)
        .set_label_area_size(LabelAreaPosition::Bottom, 40)
        .caption("Data graph", ("sans-serif", 25))
        .build_cartesian_2d(0.0..duration, 0..cpu_count-1)
        .unwrap();

    ctx.configure_mesh().disable_mesh().draw().unwrap();

    for (core, switch_events) in data {
        let mut ranges: Vec<(f64, f64, RGBColor)> = Vec::new();
        let mut prev_pos = 0.0;
        for item in switch_events.windows(2) {
            let new_pos = item[1].timestamp - item[0].timestamp;
            if let Events::SchedSwitch { old_base, ..} = &item[1].event {
                let pid = old_base.pid;
                ranges.push((prev_pos, new_pos, pid_color[&pid]));
            }
            prev_pos = new_pos;
        }
        println!("{}", core);
        dbg!(&ranges);
        
        ctx.draw_series(ranges.iter().map(|y| {
            let mut bar = Rectangle::new([
                (y.0, core), 
                (y.1, core)
            ], y.2.filled());
            bar.set_margin(4, 4, 0, 0);
            bar
        }))
        .unwrap();
    }
    // dbg!(new_data);


//     // ctx.draw_series(order.iter().zip(new_data.iter()).map(|(y, x)| {
//     //         let mut bar = Rectangle::new([
//     //             (x.0, SegmentValue::Exact(x.1[])), 
//     //             (x.0, SegmentValue::Exact(x.))
//     //         ], GREEN.filled());
//     //         bar.set_margin(1, 1, 0, 0);
//     //         bar
//     //     }))
//     //     .unwrap();


//     // let x_kps: Vec<_> = (-80..80).map(|x| x as f64 / 20.0).collect();
//     // ctx.draw_series(LineSeries::new(x_kps.iter().map(|x| (*x, x.sin())), &RED))
//     //     .unwrap();

//     // ctx.draw_series(LineSeries::new(x_kps.iter().map(|x| (*x, x.cos())), &BLUE))
//     //     .unwrap();

}