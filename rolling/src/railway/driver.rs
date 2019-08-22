use eventsim::{Process, ProcessState, EventId};
use super::infrastructure::*;
use input::staticinfrastructure::*;
use smallvec::SmallVec;
use super::dynamics::*;
use output::history::TrainLogEvent;
use super::Sim;

enum ModelContainment {
    Inside,
    Outside,
}

enum Activation {
    Wait(EventId),
    Activate,
    Running,
}

#[derive(Debug)]
struct Train {
    location: (NodeId, (Option<NodeId>, f64)),
    velocity: f64,
    params: TrainParams,
    under_train: SmallVec<[(NodeId, f64); 4]>,
}

pub struct Driver {
    id :usize,
    train: Train,
    authority: f64,
    step: (DriverAction, f64),
    connected_signals: SmallVec<[(ObjectId, f64); 4]>,
    logger: Box<Fn(TrainLogEvent)>,
    activation: Activation,
    timestep: Option<f64>,
}

impl Driver {
    pub fn new(sim: &mut Sim,
               id :usize,
               activated: EventId,
               node: NodeId,
               auth: f64,
               params: TrainParams,
               logger: Box<Fn(TrainLogEvent)>,
               timestep: Option<f64>)
               -> Self {

       //println!("INITIAL AUTHORITY {:?}", auth);
        let train = Train {
            params: params,
            location: (0, (Some(node),0.0)),
            velocity: 0.0,
            under_train: SmallVec::new(),
        };

        let d = Driver {
            id: id,
            train: train,
            authority: auth - 20.0,
            step: (DriverAction::Coast, *sim.time()),
            connected_signals: SmallVec::new(),
            logger: logger,
            activation: Activation::Wait(activated),
            timestep: timestep
        };

        d
    }

    fn activate(&mut self, sim:&mut Sim) {
        if *sim.time() > 0.0 {
            (self.logger)(TrainLogEvent::Wait(*sim.time()));
        }
        self.step = (DriverAction::Coast, *sim.time());
        self.move_train_discrete(sim);
    }

    fn goto_node(&mut self, sim: &mut Sim, node: NodeId) {
        //println!("TRAIN goto node {}", node);
        for obj in sim.world.statics.nodes[node].objects.clone() {
            if let Some(p) = sim.world.statics.objects[obj].arrive_front(node, self.id) {
                sim.start_process(p);
            }
            self.arrive_front(sim, obj);
        }
        self.train.under_train.push((node, self.train.params.length));
    }

    fn arrive_front(&mut self, sim: &Sim, obj: ObjectId) {
        match sim.world.statics.objects[obj] {
            StaticObject::Sight { distance, signal } => {
                self.connected_signals.push((signal, distance));
                (self.logger)(TrainLogEvent::Sight(signal,true));
            }
            StaticObject::Signal { .. } => {
                let log = &mut self.logger;
                self.connected_signals.retain(|&mut (s, _d)| {
                    let lost = s == obj;
                    if lost { log(TrainLogEvent::Sight(s,false)); }
                    !lost
                });
            }
            _ => {}
        }
    }

    fn move_train(&mut self, sim: &mut Sim) -> ModelContainment {
        let dt = *sim.time() - self.step.1;
        if dt <= 1e-5 {
            return ModelContainment::Inside;
        }

        self.move_train_continuous(sim);
        self.move_train_discrete(sim);

        if (self.train.location.1).0.is_none() && self.train.under_train.len() == 0 {
            ModelContainment::Outside
        } else {
            ModelContainment::Inside
        }
    }

    fn move_train_continuous(&mut self, sim :&mut Sim) {
        let (action, action_time) = self.step;
        let dt = *sim.time() - action_time;
        let update = dynamic_update(&self.train.params, self.train.velocity, 
                                    DriverPlan { action: action, dt: dt, });

        //println!("DYNAMIC UPDATE {:?}", (action,dt));
        //println!("{:?}", update);

        (self.logger)(TrainLogEvent::Move(dt, action, update));
        self.train.velocity = update.v;
        //println!("train loc {:?}", self.train.location);
        (self.train.location.1).1 -= update.dx;
        //println!("train loc {:?}", self.train.location);

        // In case there are no signals in sight,
        // the remembered authority is updated.
        self.authority -= update.dx;

        let id = self.id;
        self.train.under_train.retain(|&mut (node, ref mut dist)| {
            *dist -= update.dx;
            if *dist < 1e-5 {
                // Cleared a node.
                
                for obj in sim.world.statics.nodes[node].objects.clone() {
                    if let Some(p) = sim.world.statics.objects[obj].arrive_back(node, id) {
                        sim.start_process(p);
                    }
                }

                false
            } else {
                true
            }
        });

        {
        let log = &mut self.logger;
        self.connected_signals.retain(|&mut (obj, ref mut dist)| {
            *dist -= update.dx;
            let lost = *dist < 10.0; // If closer than 10 m, signal should already be green
                                     // and seeing a red for a very short time should be because
                                     // detector is placed in front of signal and this should not 
                                     // bother the driver.
            if lost { log(TrainLogEvent::Sight(obj, false)); } 
            !lost
        });
        }
    }

    fn move_train_discrete(&mut self, sim :&mut Sim) {
        loop {
            let (_, (end_node, dist)) = self.train.location;
            if dist > 1e-5 || end_node.is_none() { break; }

            let new_start = sim.world.statics.nodes[end_node.unwrap()].other_node;
            (self.logger)(TrainLogEvent::Node(end_node.unwrap()));
            self.goto_node(sim, new_start);
            (self.logger)(TrainLogEvent::Node(new_start));
            match sim.world.edge_from(new_start) {
                Some((Some(new_end_node), d)) => {
                    self.train.location = (new_start, (Some(new_end_node), d));
                    (self.logger)(TrainLogEvent::Edge(new_start, Some(new_end_node)));
                }
                Some((None, d)) => {
                    self.train.location = (new_start, (None, d));
                    (self.logger)(TrainLogEvent::Edge(new_start, None));
                }
                None => panic!("Derailed"),
            }
        }
    }

    fn plan_ahead(&mut self, sim: &Sim) -> DriverPlan {
        // Travel distance is limited by next node
        //println!("Travel distance is limited by next node");
        //println!("{:?}", (self.train.location.1).1);
        let mut max_dist = (self.train.location.1).1;

        // Travel distance is limited by nodes under train
        //println!("Travel distance is limited by nodes under train");
        //println!("{:?}", self.train.under_train);
        for &(_n, d) in self.train.under_train.iter() {
            max_dist = max_dist.min(d);
        }

        // Travel distance is limited by sight distances
        //println!("Travel distance is limited by sight distances");
        //println!("{:?}", self.connected_signals);
        for &(_n, d) in self.connected_signals.iter() {
            max_dist = max_dist.min(d);
        }

        // Authority is updated by signals
        for &(sig, dist) in self.connected_signals.iter() {
            match sim.world.state[sig] {
                ObjectState::Signal { ref authority } => {
                    match *authority.get() {
                        (Some(auth_dist), distant_sig) => {
                            //println!("Signal green in sight dist{} sigauth{} self.auth{}", dist, d, dist+d-20.0);
                            self.authority = dist + auth_dist + distant_sig.unwrap_or(0.0) - 20.0;
                            if self.authority < 0.0 { self.authority = 0.0; }
                        }
                        (None,_) => {
                            //println!("Signal red in sight dist{} self.auth{}", dist,dist-20.0);
                            self.authority = dist - 20.0;
                            if self.authority < 0.0 { self.authority = 0.0; }
                            break;
                        }
                    }
                }
                _ => panic!("Not a signal"),
            }
        }

        //println!("Updated authority {}", self.authority);

        // Static maximum speed profile ahead from current position
        // TODO: other speed limitations
        let static_speed_profile = StaticMaximumVelocityProfile {
            local_max_velocity: self.train.params.max_vel,
            max_velocity_ahead: SmallVec::from_slice(&[DistanceVelocity {
               dx: self.authority, v: 0.0}]),
        };

        let plan = dynamic_plan_step(&self.train.params,
                          max_dist,
                          self.train.velocity,
                          &static_speed_profile);

        //println!("PLAN: {:?} {:?} {:?} {:?} {:?} ", self.train.params, max_dist, self.train.velocity, static_speed_profile,plan);
        plan
    }
}

impl<'a> Process<Infrastructure<'a>> for Driver {
    fn resume(&mut self, sim: &mut Sim) -> ProcessState {
        match self.activation {
            Activation::Wait(ev) => {
                self.activation = Activation::Activate;
                return ProcessState::Wait(SmallVec::from_slice(&[ev]));
            },
            Activation::Activate => {
                self.activate(sim);
                self.activation = Activation::Running;
            },
            Activation::Running => { }
        };

        //println!("resume train");
        let modelcontainment = self.move_train(sim);
        match modelcontainment {
            ModelContainment::Outside => {
                //println!("TRAIN FINISHED");
                ProcessState::Finished
            },

            ModelContainment::Inside => {
                let plan = self.plan_ahead(sim);
                self.step = (plan.action, *sim.time());
                    //println!("PLAN  {:?}", plan);

                let mut events = SmallVec::new();
                if plan.dt > 1e-5 {
                    let dt = match self.timestep {
                        Some(m) => if m < plan.dt && plan.dt.is_normal() { m } else { plan.dt },
                        None => plan.dt,
                    };
                    //println!("SET TIMOUT {:?} {:?}", plan.dt, dt);
                    events.push(sim.create_timeout(dt));
                } else {
                    if self.train.velocity > 1e-5 { panic!("Velocity, but no plan."); }
                    self.train.velocity = 0.0;
                    self.step.0 = DriverAction::Coast;
                }
                //println!("Connected signals: {:?}", self.connected_signals);
                for &(ref sig, _) in self.connected_signals.iter() {
                    match sim.world.state[*sig] {
                        ObjectState::Signal { ref authority } => events.push(authority.event()),
                        _ => panic!("Object is not a signal"),
                    }
                }
                ProcessState::Wait(events)
            }
        }
    }
}
