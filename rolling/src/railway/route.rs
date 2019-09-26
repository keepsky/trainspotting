use eventsim::{Process, ProcessState, EventId, observable::*};
use super::Sim;
use smallvec::SmallVec;
use input::staticinfrastructure::*;
use super::infrastructure::*;
use output::history::{InfrastructureLogEvent, RouteStatus};
use log::*;

enum ActivateRouteState {
    Queued, // Waiting for conflicting routes to activate first
    Allocate, // Waiting for resources
    Move, // Waiting for movable elements
}

pub struct ActivateRoute {
    route: Route,
    conditions :Vec<EventId>,
    overlap: Option<usize>,
    state: ActivateRouteState,
}

impl ActivateRoute {
    pub fn new(r: Route, conditions :Vec<EventId>) -> Self {
        let overlap = if r.overlaps.len() > 0 { Some(0) } else { None };
        //println!("NEW ACTIVATE ROUTE {:?} {:?}", overlap, r);
        ActivateRoute {
            route: r,
            overlap: overlap,
            conditions: conditions,
            state: ActivateRouteState::Queued,
        }
    }
}

fn require_observable_bool_false(b :&Observable<bool>) -> Result<(), EventId> {
    if *b.get() {
        return Err(b.event());
    }
    Ok(())
}

fn require_observable_zero(b :&Observable<usize>) -> Result<(), EventId> {
    if *b.get() != 0 {
        return Err(b.event());
    }
    Ok(())
}

fn require_switch(s :ObjectId, inf :&Infrastructure) -> Result<(), EventId> {
    if let ObjectState::Switch { ref reserved, .. } = inf.state[s] {
        require_observable_bool_false(&reserved)?;
    } else {
        panic!("Not a switch.");
    }
    Ok(())
}

fn require_tvd(s :ObjectId, endpoint :Option<ObjectId>, inf :&Infrastructure) -> Result<(), EventId> {
    if let ObjectState::TVDSection { ref reserved, ref occupied, .. } = inf.state[s] {

        // Require the section to be free, or previously allocated 
        // as overlap from this end point.
        match *reserved.get() {
            TVDReservation::Free => {},
            TVDReservation::Locked => return Err(reserved.event()),
            TVDReservation::Overlap(sig) => if Some(sig) != endpoint { return Err(reserved.event()); },
        };

        require_observable_zero(&occupied)?;
    } else {
        panic!("Not a TVD.");
    }
    Ok(())
}

fn unavailable_resource(r: &Route, overlap: Option<&Overlap>, infrastructure: &Infrastructure) -> Result<(),EventId> {

    let overlap_endpoint = if let RouteEntryExit::SignalTrigger { signal, .. } = r.entry { Some(signal) } else { None };

    for s in r.resources.sections.iter() {
        require_tvd(*s, overlap_endpoint, infrastructure)?;
    }

    for &(sw, _pos) in r.resources.switch_positions.iter() {
        require_switch(sw, infrastructure)?;
    }

    if let Some(overlap) = overlap {
        for s in overlap.sections.iter() {
            require_tvd(*s, overlap_endpoint, infrastructure)?;
        }

        for &(sw, _pos) in overlap.switch_positions.iter() {
            require_switch(sw, infrastructure)?;
        }
    }
    
    Ok(())
}

fn release_overlap(overlap: &Overlap, sim :&mut Sim) {
    let state = &mut sim.world.state;
    let logger = &mut sim.world.logger;
    let scheduler = &mut sim.scheduler;
    for s in overlap.sections.iter() {
        match state[*s] {
            ObjectState::TVDSection { ref mut reserved, .. } => {
                reserved.set(scheduler, TVDReservation::Free);
                logger(InfrastructureLogEvent::Reserved(*s,false));
            }
            _ => panic!("Not a TVD"),
        };
    }

    for &(sw, _pos) in overlap.switch_positions.iter() {
        match state[sw] {
            ObjectState::Switch { ref mut reserved, .. } => {
                reserved.set(scheduler, false);
                logger(InfrastructureLogEvent::Reserved(sw,false));
            }
            _ => panic!("Not a switch"),
        };
    }
}

fn allocate_overlap(overlap: &Overlap, exit :ObjectId, sim :&mut Sim) {
    let state = &mut sim.world.state;
    let logger = &mut sim.world.logger;
    let scheduler = &mut sim.scheduler;
    for s in overlap.sections.iter() {
        match state[*s] {
            ObjectState::TVDSection { ref mut reserved, .. } => {
                reserved.set(scheduler, TVDReservation::Overlap(exit));
                logger(InfrastructureLogEvent::Reserved(*s,true));
            }
            _ => panic!("Not a TVD"),
        };
    }

    for &(sw, _pos) in overlap.switch_positions.iter() {
        match state[sw] {
            ObjectState::Switch { ref mut reserved, .. } => {
                reserved.set(scheduler, true);
                logger(InfrastructureLogEvent::Reserved(sw,true));
            }
            _ => panic!("Not a switch"),
        }
    }
}

fn allocate_resources(r: &Route, sim :&mut Sim) {
    let state = &mut sim.world.state;
    let logger = &mut sim.world.logger;
    let scheduler = &mut sim.scheduler;
    for s in r.resources.sections.iter() {
        match state[*s] {
            ObjectState::TVDSection { ref mut reserved, .. } => {
                reserved.set(scheduler, TVDReservation::Locked);
                logger(InfrastructureLogEvent::Reserved(*s,true));
            }
            _ => panic!("Not a TVD"),
        };
    }

    for &(sw, _pos) in r.resources.switch_positions.iter() {
        match state[sw] {
            ObjectState::Switch { ref mut reserved, .. } => {
                reserved.set(scheduler, true);
                logger(InfrastructureLogEvent::Reserved(sw,true));
            }
            _ => panic!("Not a switch"),
        }
    }
}


fn movable_events(r: &Route, sim: &mut Sim) -> Vec<EventId> {
    let throw = r.resources.switch_positions
        .iter()
        .filter_map(|&(sw, pos)| {
            match sim.world.state[sw] {
                ObjectState::Switch { ref position, ref mut throwing, .. } => {
                    if position.get() != &Some(pos) && !throwing.is_some() {
                        Some((sw, pos))
                    } else {
                        None
                    }
                }
                _ => panic!("Not a switch"),
            }
        })
        .collect::<Vec<_>>();

    for (sw, pos) in throw {
        //println!("MOving {:?} {:?}", sw, pos);
        let throw = sim.start_process(Box::new(MoveSwitch {
            sw: sw,
            pos: pos,
            state: false,
        }));
        match sim.world.state[sw] {
            ObjectState::Switch { ref mut throwing, .. } => *throwing = Some(throw),
            _ => panic!("Not a switch"),
        };
    }

    r.resources.switch_positions
        .iter()
        .filter_map(|&(sw, _pos)| {
            match sim.world.state[sw] {
                ObjectState::Switch { ref throwing, .. } => *throwing,
                _ => panic!("Not a switch"),
            }
        })
        .collect::<Vec<_>>()

}

impl<'a> Process<Infrastructure<'a>> for ActivateRoute {
    fn resume(&mut self, sim: &mut Sim) -> ProcessState {
        let overlap = self.overlap.map(|i| self.route.overlaps[i].clone());
        (sim.world.logger)(InfrastructureLogEvent::Route(0, RouteStatus::Pending)); // TODO id from where?

        if let ActivateRouteState::Queued = self.state {
            while let Some(c) = self.conditions.pop() {
                if !sim.has_fired(c) {
                    self.conditions.push(c);
                    return ProcessState::Wait(SmallVec::from_slice(&[c]));
                }
            }
            self.state = ActivateRouteState::Allocate;
        }

        if let ActivateRouteState::Allocate = self.state {
            match unavailable_resource(&self.route, overlap.as_ref(), &sim.world) {
                Ok(()) => {
                    allocate_resources(&self.route, sim);
                    if let Some(ref overlap) = overlap { 
                        if let RouteEntryExit::Signal(end) = self.route.exit {
                            debug!("ALLOCATING OVERLAP on {:?}", self.route);
                            allocate_overlap(overlap, end, sim); 
                            if let RouteEntryExit::SignalTrigger { ref trigger_section, .. } = self.route.entry {
                                if let Some(t) = overlap.timeout {
                                    sim.start_process(Box::new(OverlapTimeout {
                                        overlap: overlap.clone(),
                                        trigger: *trigger_section,
                                        time: t,
                                        state: OverlapTimeoutState::Start,
                                    }));
                                }
                            }
                        } else {
                            panic!("Overlap has no end point.");
                        }
                    }
                    self.state = ActivateRouteState::Move;
                }
                Err(ev) => {
                    return ProcessState::Wait(SmallVec::from_slice(&[ev]));
                }
            }
        }

        // TODO smallvec5
        //println!("ROUTE movable @{}",sim.time());
        let wait_move = movable_events(&self.route, sim);
        if !wait_move.is_empty() {
            return ProcessState::Wait(wait_move.into());
        }
        //println!("ROUTE movable finished @{}",sim.time());
        //println!("Trying entry {:?}",self.route.entry);

        // Set the signal to green
        match self.route.entry {
            RouteEntryExit::SignalTrigger { ref signal, ref trigger_section } => {

                let mut distant_sig = None;
                if let RouteEntryExit::Signal(d) = &self.route.exit {
                    if let Some(StaticObject::Signal { has_distant }) = sim.world.statics.objects.get(*signal) {
                        if *has_distant {
                            distant_sig = Some(*d);
                        }
                    } 
                }

                sim.start_process(Box::new(CatchSignal {
                    signal: *signal,
                    tvd: *trigger_section,
                    state: CatchSignalState::Start,
                    distant_sig: distant_sig,
                    route_length: self.route.length,
                }));
           },
           _ =>  {},
        };

        //println!("ROUTE RELEASES: {:?}", self.route.resources.releases);
        for release in self.route.resources.releases.iter() {
            sim.start_process(Box::new(ReleaseRoute {
                trigger: release.trigger,
                resources: release.resources.clone().to_vec(),
                state: ReleaseRouteState::Start,
            }));
        }

        (sim.world.logger)(InfrastructureLogEvent::Route(0, RouteStatus::Active)); // TODO id from where?
        ProcessState::Finished
    }
}

enum OverlapTimeoutState {
    Start, AwaitTrigger, AwaitTimer,
}

struct OverlapTimeout {
    overlap: Overlap,
    trigger: ObjectId,
    time: f64,
    state: OverlapTimeoutState,
}

impl<'a> Process<Infrastructure<'a>> for OverlapTimeout {
    fn resume(&mut self, sim: &mut Sim) -> ProcessState {
        match self.state {
            OverlapTimeoutState::Start => {
                let event = match sim.world.state[self.trigger] {
                    ObjectState::TVDSection { ref mut occupied, .. } => occupied.event(),
                    _ => panic!("Not a TVD section"),
                };
                self.state = OverlapTimeoutState::AwaitTrigger;
                ProcessState::Wait(SmallVec::from_slice(&[event]))
            }
            OverlapTimeoutState::AwaitTrigger => {
                self.state = OverlapTimeoutState::AwaitTimer;
                ProcessState::Wait(SmallVec::from_slice(&[sim.create_timeout(self.time)]))
            },
            OverlapTimeoutState::AwaitTimer => {
                release_overlap(&self.overlap, sim);
                ProcessState::Finished
            },
        }
    }
}

enum CatchSignalState {
    Start,
    AwaitTrigger,
}

struct CatchSignal {
    tvd: ObjectId,
    signal: ObjectId,
    state: CatchSignalState,
    distant_sig: Option<ObjectId>,
    route_length :f64,
}
impl CatchSignal {
    fn set_auth(&self, sim :&mut Sim, auth :(Option<f64>,Option<f64>)) {
        match sim.world.state[self.signal] {
            ObjectState::Signal { ref mut authority } => {
                authority.set(&mut sim.scheduler, auth);
                (sim.world.logger)(InfrastructureLogEvent::Authority(self.signal,auth));
            }
            _ => panic!("Not a signal"),
        }
    }

    fn get_distant(&self, sim :&mut Sim) -> Option<(Option<f64>, EventId)> {
        self.distant_sig.and_then(|s| {
            match sim.world.state[s] {
                ObjectState::Signal { ref authority } => {
                    Some((authority.get().0, authority.event()))
                }, _ => panic!("not a signal"),
            }
        })
    }

    fn get_tvd_event(&self, sim :&mut Sim) -> EventId {
        match sim.world.state[self.tvd] {
            ObjectState::TVDSection { ref mut occupied, .. } => occupied.event(),
            _ => panic!("Not a TVD section"),
        }
    }

    fn was_tvd_triggered(&self, sim :&mut Sim) -> bool {
        match sim.world.state[self.tvd] {
            ObjectState::TVDSection { ref occupied, .. } => {
                *occupied.get() > 0
            }, _ => panic!("not a tvd section"),
        }
    }
}


impl<'a> Process<Infrastructure<'a>> for CatchSignal {
    fn resume(&mut self, sim: &mut Sim) -> ProcessState {
        match self.state {
            CatchSignalState::Start => {
                self.state = CatchSignalState::AwaitTrigger;
                let tvd_event = self.get_tvd_event(sim);
                if let Some((dist, dist_ev)) = self.get_distant(sim) {
                    self.set_auth(sim, (Some(self.route_length), dist));
                    ProcessState::Wait(SmallVec::from_slice(&[tvd_event, dist_ev]))
                } else {
                    self.set_auth(sim, (Some(self.route_length), None));
                    ProcessState::Wait(SmallVec::from_slice(&[tvd_event]))
                }
            }
            CatchSignalState::AwaitTrigger => {
                if self.was_tvd_triggered(sim) {
                    self.set_auth(sim, (None,None));
                    ProcessState::Finished
                } else {
                    let tvd_event = self.get_tvd_event(sim);
                    let (dist,dist_ev) = self.get_distant(sim).unwrap(); // must have distant
                                            // to be woken up without tvd triggered.
                    self.set_auth(sim, (Some(self.route_length), dist));
                    ProcessState::Wait(SmallVec::from_slice(&[tvd_event, dist_ev]))
                }
            }
        }
    }
}

enum ReleaseRouteState {
    Start,
    AwaitEntry,
    AwaitExit,
}

struct ReleaseRoute {
    trigger: ObjectId,
    resources: Vec<ObjectId>,
    state: ReleaseRouteState,
}

impl<'a> Process<Infrastructure<'a>> for ReleaseRoute {
    fn resume(&mut self, sim: &mut Sim) -> ProcessState {
        let event = match sim.world.state[self.trigger] {
            ObjectState::TVDSection { ref mut occupied, .. } => occupied.event(),
            _ => panic!("Not a TVD section"),
        };

        match self.state {
            ReleaseRouteState::Start => {
                self.state = ReleaseRouteState::AwaitEntry;
                ProcessState::Wait(SmallVec::from_slice(&[event]))
            }
            ReleaseRouteState::AwaitEntry => {
                self.state = ReleaseRouteState::AwaitExit;
                ProcessState::Wait(SmallVec::from_slice(&[event]))
            }
            ReleaseRouteState::AwaitExit => {
                for obj in &self.resources {
                    match sim.world.state[*obj] {
                        ObjectState::TVDSection { ref mut reserved, .. } => {
                            reserved.set(&mut sim.scheduler, TVDReservation::Free);
                            (sim.world.logger)(InfrastructureLogEvent::Reserved(*obj,false));
                        }
                        ObjectState::Switch { ref mut reserved, .. } => {
                            reserved.set(&mut sim.scheduler, false);
                            (sim.world.logger)(InfrastructureLogEvent::Reserved(*obj,false));
                        }
                        _ => panic!("Not a resource"),
                    };;
                }
                (sim.world.logger)(InfrastructureLogEvent::Route(0,RouteStatus::Released)); // TODO id from where? TODO partial 
                ProcessState::Finished
            }
        }
    }
}
