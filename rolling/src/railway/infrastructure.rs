use smallvec::SmallVec;
use eventsim::*;
use eventsim::observable::Observable;
use input::staticinfrastructure::*;
use output::history::InfrastructureLogEvent;

pub type TrainId = usize;
pub type InfLogger = Box<Fn(InfrastructureLogEvent)>;


use std::f64::INFINITY;
use railway::{Sim, Proc};

// pub trait Logger {
//    fn output(&mut self, msg: InfrastructureLogEvent);
//

pub trait TrainVisitable {
    fn arrive_front(&self) -> Option<Box<Proc>> {
        None
    }
    fn arrive_back(&self) -> Option<Box<Proc>> {
        None
    }
}

#[derive(Debug)]
pub enum ObjectState {
    Sight,
    Signal { authority: Observable<Option<f64>> },
    Switch {
        position: Observable<Option<SwitchPosition>>,
        throwing: Option<ProcessId>,
        reserved: Observable<bool>,
    },
    TVDSection {
        reserved: Observable<TVDReservation>,
        occupied: Observable<bool>,
    },
    TVDLimit,
}

#[derive(Debug, Copy, Clone)]
pub enum TVDReservation {
    Free,
    Locked,
    Overlap(ObjectId),
}

pub struct MoveSwitch {
    pub sw: ObjectId,
    pub pos: SwitchPosition,
    pub state: bool,
}

impl<'a> Process<Infrastructure<'a>> for MoveSwitch {
    fn resume(&mut self, sim: &mut Sim) -> ProcessState {
        if !self.state {
            self.state = true;
            ProcessState::Wait(SmallVec::from_slice(&[sim.create_timeout(5.0)]))
        } else {
            //println!("SWITCH MOVED {:?} {:?}", self.sw, self.pos);
            match sim.world.state[self.sw] {
                ObjectState::Switch { ref mut position, ref mut throwing, .. } => {
                    position.set(&mut sim.scheduler, Some(self.pos));
                    *throwing = None;
                    (sim.world.logger)(InfrastructureLogEvent::Position(self.sw, self.pos));
                }
                _ => panic!("Not a switch"),
            }
            ProcessState::Finished
        }
    }
}


#[derive(Copy, Clone)]
enum DetectEvent {
    Enter(ObjectId),
    Exit(ObjectId),
}

impl<'a> Process<Infrastructure<'a>> for DetectEvent {
    fn resume(&mut self, sim: &mut Sim) -> ProcessState {
        let infstate = &mut sim.world.state;
        let scheduler = &mut sim.scheduler;
        match *self {
            DetectEvent::Enter(obj) => {
                match infstate[obj] {
                    ObjectState::TVDSection { ref mut occupied, .. } => {
                        occupied.set(scheduler, true);
                        (sim.world.logger)(InfrastructureLogEvent::Occupied(obj, true));
                    }
                    _ => panic!("Not a TVD section"),
                }
            }
            DetectEvent::Exit(obj) => {
                match infstate[obj] {
                    ObjectState::TVDSection { ref mut occupied, .. } => {
                        occupied.set(scheduler, false);
                        (sim.world.logger)(InfrastructureLogEvent::Occupied(obj, false));
                    }
                    _ => panic!("Not a TVD section"),
                }
            }
        };
        ProcessState::Finished
    }
}

impl TrainVisitable for StaticObject {
    fn arrive_front(&self) -> Option<Box<Proc>> {
        match *self {
            StaticObject::TVDLimit { enter, .. } => {
                match enter {
                    Some(tvd) => Some(Box::new(DetectEvent::Enter(tvd))),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn arrive_back(&self) -> Option<Box<Proc>> {
        match *self {
            StaticObject::TVDLimit { exit, .. } => {
                match exit {
                    Some(tvd) => Some(Box::new(DetectEvent::Exit(tvd))),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

pub struct Infrastructure<'a> {
    pub statics: &'a StaticInfrastructure,
    pub state: Vec<ObjectState>,
    pub logger: InfLogger,
}

use std::fmt;
impl<'a> fmt::Debug for Infrastructure<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
               "Infrastructure {{ statics: {:?}, state: {:?} }}",
               self.statics,
               self.state)
    }
}


impl<'a> Infrastructure<'a> {
    pub fn new(scheduler: &mut Scheduler,
               infrastructure: &'a StaticInfrastructure,
               logger: InfLogger)
               -> Infrastructure<'a> {
        use input::staticinfrastructure::StaticObject::*;
        let state = infrastructure.objects
            .iter()
            .map(|o| match *o {
                Sight { .. } => ObjectState::Sight,
                Signal { .. } => {
                    ObjectState::Signal { authority: Observable::new(scheduler, None) }
                }
                TVDLimit { .. } => ObjectState::TVDLimit,
                TVDSection => {
                    ObjectState::TVDSection {
                        reserved: Observable::new(scheduler, TVDReservation::Free),
                        occupied: Observable::new(scheduler, false),
                    }
                }
                Switch { .. } => {
                    ObjectState::Switch {
                        position: Observable::new(scheduler, None),
                        throwing: None,
                        reserved: Observable::new(scheduler, false),
                    }
                }
            })
            .collect();
        Infrastructure {
            statics: infrastructure,
            state: state,
            logger: logger,
        }
    }

    pub fn edge_from(&self, node: NodeId) -> Option<(Option<NodeId>, f64)> {
        match self.statics.nodes[node].edges {
            Edges::Nothing => None,
            Edges::ModelBoundary => Some((None, INFINITY)),
            Edges::Single(next_node, dist) => Some((Some(next_node), dist)),
            Edges::Switchable(sw) => {
                match (&self.statics.objects[sw], &self.state[sw]) {
                    (&StaticObject::Switch { ref left_link, ref right_link, .. },
                     &ObjectState::Switch { ref position, .. }) => {
                        match *position.get() {
                            Some(SwitchPosition::Left) => Some((Some(left_link.0), left_link.1)),
                            Some(SwitchPosition::Right) => Some((Some(right_link.0), right_link.1)),
                            None => None,
                        }
                    }
                    _ => panic!("Not a switch"),
                }
            }
        }
    }
}
