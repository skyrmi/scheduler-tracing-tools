use core::panic;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum Wstate {
    Waking(u32, u32),
    Woken,
    Numa(i32, i32)
}

#[derive(Debug)]
pub enum Events {
    // unblock - exec
    SchedWaking {
        command: String,
        pid: u32,
        target_cpu: u32,
    },
    SchedWakeIdleNoIpi {
        cpu: u32,
    },
    SchedWakeup {
        command: String, 
        pid: u32,
        prev_cpu: Option<u32>,
        cpu: u32,
    },
    SchedWakeupNew {
        command: String, 
        pid: u32,
        parent_cpu: u32,
        cpu: u32,
    },
    SchedMigrateTask {
        command: String, 
        pid: u32,
        orig_cpu: u32,
        dest_cpu: u32,
        state: Wstate,
    },
    SchedSwitch {
        old_command: String, 
        old_pid: u32,
        state: String,
        new_command: String, 
        new_pid: u32,
    },

    // process lifetime
    SchedProcessFree {
        command: String, 
        pid: u32,
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
    SchedProcessWait {
        command: String, 
        pid: u32,
    },
    SchedProcessExit {
        command: String, 
        pid: u32,
    },

    // numa balancing
    SchedSwapNuma {
        src_pid: u32,
        src_cpu: i32,
        dst_pid: u32,
        dst_cpu: i32,
    },
    SchedStickNuma {
        src_pid: u32,
        src_cpu: i32,
        dst_pid: u32,
        dst_cpu: i32,
    },
    SchedMoveNuma {
        src_pid: u32,
        src_cpu: i32,
        dst_cpu: i32,
    },
    // other
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

pub fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

pub struct TraceParser {
    pub cpu_count: u32,
    pub first_timestamp: Option<f64>,
    pub last_timestamp: Option<f64>,
    lines: io::Lines<io::BufReader<File>>,
    process_state: HashMap<u32, Wstate>,
}

impl TraceParser {
    pub fn new(filepath: &str) -> Self {
        let file = File::open(filepath).expect("Failed to open file");
        let reader = io::BufReader::new(file);
        let mut lines = reader.lines();

        let cpu_count = if let Some(Ok(line)) = lines.next() {
            let part: Vec<&str> = line.split_whitespace().collect();
            if part.len() > 0 && part[0].contains("cpus=") {
                part[0].replace("cpus=", "").parse().unwrap()
            } else {
                panic!("Invalid format: Expected 'cpus=' in the first line");
            }
        } else {
            panic!("Unable to read trace");
        };

        TraceParser {
            cpu_count,
            first_timestamp: None,
            last_timestamp: None,
            lines,
            process_state: HashMap::new(),
        }
    }

    pub fn next_action(&mut self) -> Option<(Action, &HashMap<u32, Wstate>, Option<f64>)> {
        while let Some(Ok(line)) = self.lines.next() {
            let part: Vec<&str> = line.split_whitespace().collect();
            if part.len() > 2 {
                let action = get_action(&part, &mut self.process_state);
                if self.first_timestamp.is_none() {
                    self.first_timestamp = Some(action.timestamp);
                }
                self.last_timestamp = Some(action.timestamp);
                return Some((action, &self.process_state, self.first_timestamp));
            }
        }
        None
    }

}

fn extract_command_and_pid(parts: &[&str], sep: char, n: usize) -> (String, u32, usize) {
    let mut command = String::new();
    let mut pid = 0;
    let mut next_index = n;

    for (index, part) in parts.iter().skip(n).enumerate() {
            if let Some((base, suffix)) = part.rsplit_once(sep) {
                if let Ok(p) = suffix.parse::<u32>() {
                    if index != 0 {
                        command.push(' ');
                    }
                    command.push_str(base);
                    pid = p;
                    next_index += index;
                    break;
                }
        }
            command.push(' ');
            command.push_str(part);
        }
    (command, pid, next_index)
}

fn parse_named_args(parts: &[&str], position: usize, comm: &str, id: &str) -> (String, u32, usize) {
    let mut command = String::new();
    command.push_str(parts[position].replace(comm, "").as_str());
    let mut position = position + 1;

    for (index, part) in parts.iter().skip(position).enumerate() {
        if part.starts_with(id) { 
            position += index;
            break; 
        }
        command.push(' ');
        command.push_str(part);
    }

    let pid: u32 = parts[position].replace(id, "").parse().unwrap();
    (command, pid, position)
}

fn get_event(part: &Vec<&str>, _process_pid: u32, process_cpu: u32, process_state: &mut HashMap<u32, Wstate>, event_type: &str, index: usize) -> Events {
    match event_type {
        "sched_waking" => {
            let (command, pid, index) = parse_named_args(&part, index, "comm=", "pid=");
            let target_cpu: u32 = String::from(part[index + 2]).replace("target_cpu=", "").parse().unwrap();

            process_state.insert(pid, Wstate::Waking(process_cpu, target_cpu));
            Events::SchedWaking { command, pid, target_cpu }
        }
        "sched_wake_idle_without_ipi" => {
            let cpu = String::from(part[index]).replace("cpu=", "").parse().unwrap();
            Events::SchedWakeIdleNoIpi { cpu }
        }
        "sched_wakeup" => {
            let (command, pid, index) = extract_command_and_pid(part, ':', index);
            let cpu: u32 = String::from(part[index + 2]).replace("CPU:", "").parse().unwrap();

            let mut prev_cpu: Option<u32> = None;
            if process_state.contains_key(&pid) {
                if let Wstate::Waking(old_cpu, _) = process_state[&pid] {
                    prev_cpu = Some(old_cpu);
                }
            }
            process_state.insert(pid, Wstate::Woken);
            Events::SchedWakeup { command, pid, prev_cpu, cpu }
        }
        "sched_wakeup_new" => {
            let (command, pid, index) = extract_command_and_pid(part, ':', index);
            let cpu: u32 = String::from(part[index + 2]).replace("CPU:", "").parse().unwrap();

            let mut parent_cpu = cpu;
            if process_state.contains_key(&pid) {
                if let Wstate::Waking(_, parent) = process_state[&pid] {
                    parent_cpu = parent;
                } 
                else {
                    panic!("Wakeup without fork");
                }
            }
            process_state.insert(pid, Wstate::Woken);
            Events::SchedWakeupNew { command, pid, parent_cpu, cpu }
        }
        "sched_migrate_task" => {
            let (command, pid, index) = parse_named_args(&part, index, "comm=", "pid=");
            let orig_cpu: u32 = String::from(part[index + 2]).replace("orig_cpu=", "").parse().unwrap();
            let dest_cpu: u32 = String::from(part[index + 3]).replace("dest_cpu=", "").parse().unwrap();

            let mut temp = Wstate::Woken;
            if process_state.contains_key(&pid) {
                temp = process_state[&pid];
            }
            let mut state = temp;
            if let Wstate::Numa(c1, c2) = temp {
                if orig_cpu != c1 as u32 || dest_cpu != c2 as u32 {
                    state = Wstate::Woken;
                }
            }
            Events::SchedMigrateTask { command, pid, orig_cpu, dest_cpu, state}
        }
        "sched_switch" => {
            let (old_command, old_pid, index) = extract_command_and_pid(part, ':', index);
            let state = part[index + 2];
            let (new_command, new_pid, ..) = extract_command_and_pid(part, ':', index + 4);
            
            Events::SchedSwitch { old_command, old_pid, state: String::from(state), new_command, new_pid }
        },
        "sched_process_free" => {
            let (command, pid, ..) = parse_named_args(&part, index, "comm=", "pid=");

            Events::SchedProcessFree { command, pid }
        },
        "sched_process_exec" => {
            let index = index;
            let filename = String::from(part[index]).replace("filename=", "");
            let pid: u32 = String::from(part[index + 1]).replace("pid=", "").parse().unwrap();
            let old_pid: u32 = String::from(part[index + 2]).replace("old_pid=", "").parse().unwrap();
            Events::SchedProcessExec { filename, pid, old_pid }
        },
        "sched_process_fork" => {
            let (command, pid, index) = parse_named_args(&part, index, "comm=", "pid=");
            let (child_command, child_pid, ..) = parse_named_args(&part, index + 1, "child_comm=", "child_pid=");

            process_state.insert(child_pid, Wstate::Waking(process_cpu, process_cpu));
            Events::SchedProcessFork { command, pid, child_command, child_pid }
        },
        "sched_process_wait" => {
            let (command, pid, ..) = parse_named_args(&part, index, "comm=", "pid=");
            Events::SchedProcessWait { command, pid }
        },
        "sched_process_exit" => {
            let (command, pid, ..) = parse_named_args(&part, index, "comm=", "pid=");
            Events::SchedProcessExit { command, pid }
        },
        "sched_swap_numa" => {
            let src_pid: u32 = String::from(part[index]).replace("src_pid=", "").parse().unwrap();
            let src_cpu: i32 = String::from(part[index + 3]).replace("src_cpu=", "").parse().unwrap();

            let dst_pid: u32 = String::from(part[index + 5]).replace("dst_pid=", "").parse().unwrap();
            let dst_cpu: i32 = String::from(part[index + 8]).replace("dst_cpu=", "").parse().unwrap();

            process_state.insert(src_pid, Wstate::Numa(src_cpu, dst_cpu));
            process_state.insert(dst_pid, Wstate::Numa(dst_cpu, src_cpu));
            Events::SchedSwapNuma { src_pid, src_cpu, dst_pid, dst_cpu }
        }
        "sched_stick_numa" => {
            let src_pid: u32 = String::from(part[index]).replace("src_pid=", "").parse().unwrap();
            let src_cpu: i32 = String::from(part[index + 3]).replace("src_cpu=", "").parse().unwrap();

            let dst_pid: u32 = String::from(part[index + 5]).replace("dst_pid=", "").parse().unwrap();
            let dst_cpu: i32 = String::from(part[index + 8]).replace("dst_cpu=", "").parse().unwrap();

            Events::SchedStickNuma { src_pid, src_cpu, dst_pid, dst_cpu }
        },
        "sched_move_numa" => {
            let src_pid: u32 = String::from(part[index]).replace("pid=", "").parse().unwrap();
            let src_cpu: i32 = String::from(part[index + 3]).replace("src_cpu=", "").parse().unwrap();
            let dst_cpu: i32 = String::from(part[index + 5]).replace("dst_cpu=", "").parse().unwrap();

            process_state.insert(src_pid, Wstate::Numa(src_cpu, dst_cpu));
            Events::SchedMoveNuma { src_pid, src_cpu, dst_cpu }
        }
        _ => Events::NotSupported
    }
}

pub fn get_action(part: &Vec<&str>, process_state: &mut HashMap<u32, Wstate>) -> Action {
    let (process, pid, index) = extract_command_and_pid(part, '-', 0);
    let cpu: u32 = String::from(part[index + 1]).replace(&['[', ']'][..], "").parse().unwrap();

    let mut timestamp = String::from(part[index + 2]);
    timestamp.pop();
    let timestamp: f64 = timestamp.parse().unwrap();
    
    let mut event_type = String::from(part[index + 3]);
    event_type.pop();
    
    let event = get_event(part, pid, cpu, process_state, &event_type, index + 4);
    Action {process, pid, cpu, timestamp, event}
}
