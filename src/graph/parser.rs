use core::panic;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;

#[derive(Debug)]
pub struct Base {
    pub command: String,
    pub pid: u32,
    priority: i32, 
}

#[derive(Debug)]
pub struct NumaArgs {
    pub pid: u32,
    pub tgid: u32,
    pub ngid: u32,
    pub cpu: i32,
    pub nid: i32
}

#[derive(Debug)]
pub enum Events {
    SchedWaking {
        base: Base,
        target_cpu: u32,
    },
    SchedWakeIdleNoIpi {
        cpu: u32,
    },
    SchedWakeup {
        base: Base,
        cpu: u32,
    },
    SchedMigrateTask {
        base: Base,
        orig_cpu: u32,
        dest_cpu: u32,
    },
    SchedSwitch {
        old_base: Base,
        state: String,
        new_base: Base,
    },
    SchedProcessFree {
        base: Base,
    },
    SchedProcessExec {
        filename: String,
        pid: u32,
        old_pid: u32,
    },
    SchedProcessExit {
        base: Base
    },
    SchedSwapNuma {
        src: NumaArgs,
        dest: NumaArgs
    },
    SchedStickNuma {
        src: NumaArgs,
        dest: NumaArgs
    },

    Empty
}

#[derive(Debug)]
pub struct Action {
    pub process: String,
    pub cpu: u32,
    pub timestamp: f64,
    pub event: Events,
}


fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

fn get_event(part: &Vec<&str>, event_type: &str) -> Events {
    match event_type {
        "sched_waking" => {
            let mut index = 4;
            let command = String::from(part[index]).replace("comm=", "");

            if part[index + 1].starts_with("pid=") == false {
                index += 1;
            }
            let pid: u32 = String::from(part[index + 1]).replace("pid=", "").parse().unwrap();
            let priority: i32 = String::from(part[index + 2]).replace("prio=", "").parse().unwrap();
            let target_cpu: u32 = String::from(part[index + 3]).replace("target_cpu=", "").parse().unwrap();

            let base = Base { command, pid, priority };
            Events::SchedWaking { base, target_cpu }
        }
        "sched_wake_idle_without_ipi" => {
            let cpu = String::from(part[4]).replace("cpu=", "").parse().unwrap();
            Events::SchedWakeIdleNoIpi { cpu }
        }
        "sched_wakeup" => {
            // let mut index = 4;
            // let mut command: Vec<&str> = part[index].split(":").collect();
            // let pid: u32 = String::from(command.pop().unwrap()).parse().unwrap();
            // let command = String::from(command.remove(0)).parse().unwrap();
            // let priority: i32 = String::from(part[index + 1]).replace(&['[', ']'][..], "").parse().unwrap();
            // let cpu: u32 = String::from(part[index + 2]).replace("CPU:", "").parse().unwrap();

            // let base = Base { command, pid, priority };
            // Events::SchedWakeup { base, cpu }

            let mut index = 4;
            let mut command: Vec<&str> = part[index].split(":").collect();
            let pid: u32;
            if String::from(command.pop().unwrap()).parse().is_err() {
                index += 1;
                command = part[index].split(":").collect();
                pid = String::from(command.pop().unwrap()).parse().unwrap();
            }
            let command = String::from(command.remove(0)).parse().unwrap();
            let priority: i32 = String::from(part[index + 1]).replace(&['[', ']'][..], "").parse().unwrap();
            let cpu: u32 = String::from(part[index + 2]).replace("CPU:", "").parse().unwrap();
            let base = Base { command, pid, priority };
            Events::SchedWakeup { base, cpu }
        }
        "sched_migrate_task" => {
            let command = String::from(part[4]).replace("comm=", "");
            let pid: u32 = String::from(part[5]).replace("pid=", "").parse().unwrap();
            let priority: i32 = String::from(part[6]).replace("prio=", "").parse().unwrap();
            let orig_cpu: u32 = String::from(part[7]).replace("orig_cpu=", "").parse().unwrap();
            let dest_cpu: u32 = String::from(part[8]).replace("dest_cpu=", "").parse().unwrap();

            let base = Base { command, pid, priority };
            Events::SchedMigrateTask { base, orig_cpu, dest_cpu }
        }
        "sched_switch" => {
            let mut old_command: Vec<&str> = part[4].split(":").collect();
            let old_pid: u32 = String::from(old_command.pop().unwrap()).parse().unwrap();
            let old_command = String::from(old_command.remove(0)).parse().unwrap();
            let old_priority: i32 = String::from(part[5]).replace(&['[', ']'][..], "").parse().unwrap();
            
            let state = String::from(part[6]);

            let mut new_command: Vec<&str> = part[8].split(":").collect();
            let new_pid: u32 = String::from(new_command.pop().unwrap()).parse().unwrap();
            let new_command = String::from(new_command.remove(0)).parse().unwrap();
            let new_priority: i32 = String::from(part[9]).replace(&['[', ']'][..], "").parse().unwrap();

            let old_base = Base { command: old_command, pid: old_pid, priority: old_priority };
            let new_base = Base { command: new_command, pid: new_pid, priority: new_priority };

            Events::SchedSwitch { old_base, state, new_base }
        },
        "sched_process_free" => {
            let command = String::from(part[4]).replace("comm=", "");
            let pid: u32 = String::from(part[5]).replace("pid=", "").parse().unwrap();
            let priority: i32 = String::from(part[6]).replace("prio=", "").parse().unwrap();

            let base = Base { command, pid, priority };
            Events::SchedProcessFree { base }
        },
        "sched_process_exit" => {
            let command = String::from(part[4]).replace("comm=", "");
            let pid: u32 = String::from(part[5]).replace("pid=", "").parse().unwrap();
            let priority: i32 = String::from(part[6]).replace("prio=", "").parse().unwrap();

            let base = Base { command, pid, priority };
            Events::SchedProcessExit { base }
        },
        "sched_swap_numa" => {
            let pid: u32 = String::from(part[4]).replace("src_pid=", "").parse().unwrap();
            let tgid: u32 = String::from(part[5]).replace("src_tgid=", "").parse().unwrap();
            let ngid: u32 = String::from(part[6]).replace("src_ngid=", "").parse().unwrap();
            let cpu: i32 = String::from(part[7]).replace("src_cpu=", "").parse().unwrap();
            let nid: i32 = String::from(part[8]).replace("src_nid=", "").parse().unwrap();

            let src = NumaArgs { pid, tgid, ngid, cpu, nid };

            let pid: u32 = String::from(part[9]).replace("dst_pid=", "").parse().unwrap();
            let tgid: u32 = String::from(part[10]).replace("dst_tgid=", "").parse().unwrap();
            let ngid: u32 = String::from(part[11]).replace("dst_ngid=", "").parse().unwrap();
            let cpu: i32 = String::from(part[12]).replace("dst_cpu=", "").parse().unwrap();
            let nid: i32 = String::from(part[13]).replace("dst_nid=", "").parse().unwrap();

            let dest = NumaArgs { pid, tgid, ngid, cpu, nid };
            Events::SchedSwapNuma { src, dest }
        }
        "sched_stick_numa" => {
            let pid: u32 = String::from(part[4]).replace("src_pid=", "").parse().unwrap();
            let tgid: u32 = String::from(part[5]).replace("src_tgid=", "").parse().unwrap();
            let ngid: u32 = String::from(part[6]).replace("src_ngid=", "").parse().unwrap();
            let cpu: i32 = String::from(part[7]).replace("src_cpu=", "").parse().unwrap();
            let nid: i32 = String::from(part[8]).replace("src_nid=", "").parse().unwrap();

            let src = NumaArgs { pid, tgid, ngid, cpu, nid };

            let pid: u32 = String::from(part[9]).replace("dst_pid=", "").parse().unwrap();
            let tgid: u32 = String::from(part[10]).replace("dst_tgid=", "").parse().unwrap();
            let ngid: u32 = String::from(part[11]).replace("dst_ngid=", "").parse().unwrap();
            let cpu: i32 = String::from(part[12]).replace("dst_cpu=", "").parse().unwrap();
            let nid: i32 = String::from(part[13]).replace("dst_nid=", "").parse().unwrap();

            let dest = NumaArgs { pid, tgid, ngid, cpu, nid };
            Events::SchedStickNuma { src, dest }
        }
        _ => Events::Empty
    }
}

fn get_action(part: &Vec<&str>) -> Action {
    let process = String::from(part[0]);
    let cpu: u32 = String::from(part[1]).replace(&['[', ']'][..], "").parse().unwrap();
    let mut timestamp = String::from(part[2]);
    timestamp.pop();
    let timestamp: f64 = timestamp.parse().unwrap();
    let mut event_type = String::from(part[3]);
    event_type.pop();

    let event = get_event(part, &event_type);

    // actions.insert(process.clone(), Action {process, cpu, timestamp, event });
    Action {process, cpu, timestamp, event}
}

pub fn parse_file() -> (u32, Vec<Action>) {
    if let Ok(mut lines) = read_lines("./src/main_report.dat") {
        let cpu_count: u32 = lines.next().unwrap().expect("Unable to read cpu count").replace("cpus=", "").parse().unwrap();
        let mut actions: Vec<Action> = Vec::new();

        let mut line_no = 2;
        for line in lines {
            if let Ok(ip) = line {
                let part: Vec<&str> = ip.split_whitespace().collect();
                actions.push(get_action(&part));
                println!("{} {:?} \n", line_no, actions.last());
                line_no += 1;
            }
        }
        (cpu_count, actions)
    }
    else {
        panic!("Failed to read trace");
    }
}
