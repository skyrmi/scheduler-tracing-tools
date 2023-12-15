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
    SchedProcessFork {
        command: String,
        pid: u32,
        child_command: String,
        child_pid: u32,
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
    SchedMoveNuma {
        src: NumaArgs,
        dest: NumaArgs,
    },
    NotSupported
}

#[derive(Debug)]
pub struct Action {
    pub process: String,
    pub pid: u32,
    pub cpu: u32,
    pub timestamp: f64,
    pub event: Events,
}


fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

fn get_event(part: &Vec<&str>, event_type: &str, index: usize) -> Events {
    match event_type {
        "sched_waking" => {
            let mut index = index + 4;
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
            let cpu = String::from(part[index + 4]).replace("cpu=", "").parse().unwrap();
            Events::SchedWakeIdleNoIpi { cpu }
        }
        "sched_wakeup" => {
            let mut index = index + 4;
            let mut command: Vec<&str> = part[index].split(":").collect();
            let pid: u32;
            let temp = String::from(command.pop().unwrap()).parse::<u32>();
            if temp.is_err() {
                index += 1;
                command = part[index].split(":").collect();
                pid = String::from(command.pop().unwrap()).parse::<u32>().unwrap();
                command.insert(0, part[index - 1]);
                command.join(" ");
            } 
            else {
                pid = temp.unwrap();
            }
            let command = String::from(command.remove(0)).parse().unwrap();
            let priority: i32 = String::from(part[index + 1]).replace(&['[', ']'][..], "").parse().unwrap();
            let cpu: u32 = String::from(part[index + 2]).replace("CPU:", "").parse().unwrap();
            let base = Base { command, pid, priority };
            Events::SchedWakeup { base, cpu }
        }
        "sched_migrate_task" => {
            let index = index + 4;
            let command = String::from(part[index]).replace("comm=", "");
            let pid: u32 = String::from(part[index + 1]).replace("pid=", "").parse().unwrap();
            let priority: i32 = String::from(part[index + 2]).replace("prio=", "").parse().unwrap();
            let orig_cpu: u32 = String::from(part[index + 3]).replace("orig_cpu=", "").parse().unwrap();
            let dest_cpu: u32 = String::from(part[index + 4]).replace("dest_cpu=", "").parse().unwrap();

            let base = Base { command, pid, priority };
            Events::SchedMigrateTask { base, orig_cpu, dest_cpu }
        }
        "sched_switch" => {
            let orig_index = index;
            let mut index = index + 4;
            let mut old_command: Vec<&str> = part[index].split(":").collect();
            let old_pid: u32;
            let temp = String::from(old_command.pop().unwrap()).parse::<u32>();
            if temp.is_err() {
                index += 1;
                old_command = part[index].split(":").collect();
                old_pid = String::from(old_command.pop().unwrap()).parse::<u32>().unwrap();
                // let old_command = old_command.join(":");
                old_command.insert(0, part[index - 1]);
            } 
            else {
                old_pid = temp.unwrap();
            }
            let old_command = old_command.join(" ");
            // let old_command = String::from(old_command.remove(0)).parse().unwrap();
            let old_priority: i32 = String::from(part[index + 1]).replace(&['[', ']'][..], "").parse().unwrap();
            
            let state = String::from(part[index + 2]);


            let mut index = orig_index + 8;
            let mut new_command: Vec<&str> = part[index].split(":").collect();
            let new_pid: u32;
            let temp = String::from(new_command.pop().unwrap()).parse::<u32>();
            if temp.is_err() {
                index += 1;
                new_command = part[index].split(":").collect();
                new_pid = String::from(new_command.pop().unwrap()).parse::<u32>().unwrap();
                new_command.insert(0, part[index - 1]);
            } 
            else {
                new_pid = temp.unwrap();
            }
            // println!("{:?}", new_command);
            let new_command = new_command.join(" ");
            // let new_command = String::from(new_command.remove(0)).parse().unwrap();
            let new_priority: i32 = String::from(part[index + 1]).replace(&['[', ']'][..], "").parse().unwrap();

            let old_base = Base { command: old_command, pid: old_pid, priority: old_priority };
            let new_base = Base { command: new_command, pid: new_pid, priority: new_priority };

            Events::SchedSwitch { old_base, state, new_base }
        },
        "sched_process_free" => {
            let index = index + 4;
            let command = String::from(part[index]).replace("comm=", "");
            let pid: u32 = String::from(part[index + 1]).replace("pid=", "").parse().unwrap();
            let priority: i32 = String::from(part[index + 2]).replace("prio=", "").parse().unwrap();

            let base = Base { command, pid, priority };
            Events::SchedProcessFree { base }
        },
        "sched_process_exec" => {
            let index = index + 4;
            let filename = String::from(part[index]).replace("filename=", "");
            let pid: u32 = String::from(part[index + 1]).replace("pid=", "").parse().unwrap();
            let old_pid: u32 = String::from(part[index + 2]).replace("old_pid=", "").parse().unwrap();
            Events::SchedProcessExec { filename, pid, old_pid }
        },
        "sched_process_fork" => {
            let index = index + 4;
            let command = String::from(part[index]).replace("comm=", "");
            let pid: u32 = String::from(part[index + 1]).replace("pid=", "").parse().unwrap();
            let child_command = String::from(part[index + 2]).replace("child_comm=", "");
            let child_pid: u32 = String::from(part[index + 3]).replace("child_pid=", "").parse().unwrap();
            Events::SchedProcessFork { command, pid, child_command, child_pid }
        },
        "sched_process_exit" => {
            let index = index + 4;
            let command = String::from(part[index]).replace("comm=", "");
            let pid: u32 = String::from(part[index + 1]).replace("pid=", "").parse().unwrap();
            let priority: i32 = String::from(part[index + 2]).replace("prio=", "").parse().unwrap();

            let base = Base { command, pid, priority };
            Events::SchedProcessExit { base }
        },
        "sched_swap_numa" => {
            let index = index + 4;
            let pid: u32 = String::from(part[index]).replace("src_pid=", "").parse().unwrap();
            let tgid: u32 = String::from(part[index + 1]).replace("src_tgid=", "").parse().unwrap();
            let ngid: u32 = String::from(part[index + 2]).replace("src_ngid=", "").parse().unwrap();
            let cpu: i32 = String::from(part[index + 3]).replace("src_cpu=", "").parse().unwrap();
            let nid: i32 = String::from(part[index + 4]).replace("src_nid=", "").parse().unwrap();

            let src = NumaArgs { pid, tgid, ngid, cpu, nid };

            let pid: u32 = String::from(part[index + 5]).replace("dst_pid=", "").parse().unwrap();
            let tgid: u32 = String::from(part[index + 6]).replace("dst_tgid=", "").parse().unwrap();
            let ngid: u32 = String::from(part[index + 7]).replace("dst_ngid=", "").parse().unwrap();
            let cpu: i32 = String::from(part[index + 8]).replace("dst_cpu=", "").parse().unwrap();
            let nid: i32 = String::from(part[index + 9]).replace("dst_nid=", "").parse().unwrap();

            let dest = NumaArgs { pid, tgid, ngid, cpu, nid };
            Events::SchedSwapNuma { src, dest }
        }
        "sched_stick_numa" => {
            let index = index + 4;
            let pid: u32 = String::from(part[index]).replace("src_pid=", "").parse().unwrap();
            let tgid: u32 = String::from(part[index + 1]).replace("src_tgid=", "").parse().unwrap();
            let ngid: u32 = String::from(part[index + 2]).replace("src_ngid=", "").parse().unwrap();
            let cpu: i32 = String::from(part[index + 3]).replace("src_cpu=", "").parse().unwrap();
            let nid: i32 = String::from(part[index + 4]).replace("src_nid=", "").parse().unwrap();

            let src = NumaArgs { pid, tgid, ngid, cpu, nid };

            let pid: u32 = String::from(part[index + 5]).replace("dst_pid=", "").parse().unwrap();
            let tgid: u32 = String::from(part[index + 6]).replace("dst_tgid=", "").parse().unwrap();
            let ngid: u32 = String::from(part[index + 7]).replace("dst_ngid=", "").parse().unwrap();
            let cpu: i32 = String::from(part[index + 8]).replace("dst_cpu=", "").parse().unwrap();
            let nid: i32 = String::from(part[index + 9]).replace("dst_nid=", "").parse().unwrap();

            let dest = NumaArgs { pid, tgid, ngid, cpu, nid };
            Events::SchedStickNuma { src, dest }
        },
        _ => Events::NotSupported
    }
}

fn get_action(part: &Vec<&str>) -> Action {
    let mut index = 0;
    let mut process: Vec<&str> = part[index].split("-").collect();
    let pid: u32;
    let temp = String::from(process.pop().unwrap()).parse::<u32>();
    if temp.is_err() {
        index += 1;
        process = part[index].split("-").collect();
        pid = String::from(process.pop().unwrap()).parse::<u32>().unwrap();
        // let old_command = old_command.join(":");
        process.insert(0, part[index - 1]);
    } 
    else {
        pid = temp.unwrap();
    }
    let process = process.join("-");

    let cpu: u32 = String::from(part[index + 1]).replace(&['[', ']'][..], "").parse().unwrap();
    let mut timestamp = String::from(part[index + 2]);
    timestamp.pop();
    let timestamp: f64 = timestamp.parse().unwrap();
    let mut event_type = String::from(part[index + 3]);
    event_type.pop();

    let event = get_event(part, &event_type, index);

    // actions.insert(process.clone(), Action {process, cpu, timestamp, event });
    Action {process, pid, cpu, timestamp, event}
}

pub fn parse_file() -> (u32, Vec<Action>) {
    if let Ok(mut lines) = read_lines("./input/main_report.dat") {
        let cpu_count: u32 = lines.next().unwrap().expect("Unable to read cpu count").replace("cpus=", "").parse().unwrap();
        let mut actions: Vec<Action> = Vec::new();

        // let mut line_no = 2;
        for line in lines {
            if let Ok(ip) = line {
                let part: Vec<&str> = ip.split_whitespace().collect();
                actions.push(get_action(&part));
                // println!("{} {:?} \n", line_no, actions.last());
                // line_no += 1;
            }
        }
        // dbg!(&actions);
        (cpu_count, actions)
    }
    else {
        panic!("Failed to read trace");
    }
}
