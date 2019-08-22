use input::staticinfrastructure::{SwitchPosition, InfNames};
use railway::dynamics::{DriverAction, DistanceVelocity, TrainParams};
use failure;
use input::staticinfrastructure::NodeId;

#[derive(Debug)]
pub struct History {
    pub inf: Vec<InfrastructureLogEvent>,
    pub trains: Vec<(String, TrainParams, Vec<TrainLogEvent>)>,
}

impl Default for History {
    fn default() -> History { 
        History { inf: vec![], trains: vec![] }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum RouteStatus {
    Pending, Active, Released,
}

#[derive(Debug)]
pub enum InfrastructureLogEvent {
    Wait(f64),
    Route(usize,RouteStatus), // TODO route identification is wrong?
    Authority(usize, (Option<f64>, Option<f64>)), // signal objectid
    Reserved(usize, bool), // tvd objectid
    Occupied(usize, bool, NodeId, usize), // tvd objectid
    Position(usize, SwitchPosition), // switch objectid
}

#[derive(Debug)]
pub enum TrainLogEvent {
    Wait(f64),
    Node(usize), // refer to nodeid
    Edge(usize, Option<usize>), // refer to two nodeid. if the second one is None then train is exiting model
    Sight(usize, bool), // has sight to signal objectid
    Move(f64, DriverAction, DistanceVelocity), 
}

/// Print one train node visits per line on the following format:
/// `trainname time nodename`.
pub fn visits(inf :&InfNames<String>, h: &History) -> Result<String,failure::Error> {
    use std::fmt::Write;
    let mut s = String::new();
    for &(ref train_name, ref _params, ref events) in &h.trains {
        let mut t = 0.0;
        for ev in events {
            use self::TrainLogEvent::*;
            match *ev {
                Wait(dt) => t += dt,
                Move(dt,_,_) => t += dt,

                Node(x) => {
                    let node_name = inf.node_names.
                        iter().find(|&(_k,v)| v == &x).expect("unknown node").0;
                    write!(s, "{} {} {}\n", train_name, t, node_name)?;
                },

                _ => {},
            }
        }
    }
    Ok(s)
}
