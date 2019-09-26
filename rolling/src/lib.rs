extern crate log;
extern crate smallvec;
extern crate ordered_float;
extern crate regex;
extern crate failure;
#[macro_use] extern crate failure_derive;

pub mod input;
pub mod output;
pub mod eventsim;
pub mod railway;

pub mod ffi;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::cell::RefCell;
use output::history::InfrastructureLogEvent;
use std::hash::Hash;
use std::fmt::Debug;

pub fn evaluate_plan<RouteRef : Hash + Eq + Debug + Clone >
                    (staticinfrastructure: &input::staticinfrastructure::StaticInfrastructure,
                     //names: &input::staticinfrastructure::InfNames<InfRef>,
                     routes: &HashMap<RouteRef,input::staticinfrastructure::Route>,
                     dispatch: &input::dispatch::Dispatch<RouteRef>, 
                     timestep :Option<f64>) -> output::history::History {


    let mut train_logs = Vec::new();
    let inf_log = Rc::new(RefCell::new(Vec::new()));
    let time_log = {
        let log = inf_log.clone();
        Box::new(move |t| if t > 0.0 { log.borrow_mut().push(InfrastructureLogEvent::Wait(t)); })
    };

    let mut scheduler = eventsim::Scheduler::new();
    let world_log = inf_log.clone();
    let world = railway::infrastructure::Infrastructure::new(
        &mut scheduler, staticinfrastructure, 
        Box::new(move |i| world_log.borrow_mut().push(i))); 
    let mut sim = eventsim::Simulation::new_with_scheduler(world, scheduler);
    sim.set_time_log(time_log);

    let mut resource_routes : HashMap<input::staticinfrastructure::ObjectId, HashSet<RouteRef>> = HashMap::new();
    for (name,route) in routes.iter() {
        let objs = route.resources.sections.iter().chain(
            route.resources.switch_positions.iter().map(|(x,_)| x));
        for obj in objs {
            resource_routes.entry(*obj).or_insert(HashSet::new()).insert(name.clone());
        }
    }
    fn get_conflicting_routes<RouteRef : Hash + Clone + Eq>(res :&HashMap<input::staticinfrastructure::ObjectId,HashSet<RouteRef>>, r :&input::staticinfrastructure::Route) -> HashSet<RouteRef> {
        let mut set = HashSet::new();
        let objs = r.resources.sections.iter().chain(
            r.resources.switch_positions.iter().map(|(x,_)| x));
        for o in objs {
            if let Some(obj_route_set) = res.get(o) {
                set.extend(obj_route_set.iter().cloned());
            }
        }
        set
    }

    let mut pending_routes = HashMap::new();

    for action in &dispatch.actions {
        use input::dispatch::DispatchAction::*;
        match *action {
            Wait(Some(t)) => sim.advance_by(t),
            Wait(None) =>  {
                for (_r,e) in pending_routes.drain() {
                    sim.advance_to(e);
                }
            },
            Route(ref route_name) => match routes.get(route_name) {

                Some(route) => {
                    let mut conflict_events = Vec::new();
                    for conflicting_name in get_conflicting_routes(&resource_routes, route) {
                        if let Some(ev) = pending_routes.get(&conflicting_name) {
                            conflict_events.push(*ev);
                        }
                    }

                    let activated = sim.start_process(Box::new(
                        railway::route::ActivateRoute::new(route.clone(), conflict_events)));
                    pending_routes.insert(route_name.clone(),activated);
                },
                _ => panic!("Unknown route \"{:?}\"", route_name),
            },
            Train(ref name, ref params, ref route_name) =>  {
                let (activated, node_idx, auth_dist) = match routes.get(route_name) {
                    Some(route) => {

                        let mut conflict_events = Vec::new();
                        for conflicting_name in get_conflicting_routes(&resource_routes, route) {
                            if let Some(ev) = pending_routes.get(&conflicting_name) {
                                conflict_events.push(*ev);
                            }
                        }

                        match route.entry {
                            staticinfrastructure::RouteEntryExit::Boundary(Some(id)) => {

                                let activated = sim.start_process(Box::new(
                                    railway::route::ActivateRoute::new(route.clone(),
                                    conflict_events)));
                                pending_routes.insert(route_name.clone(), activated);

                                (activated, id, route.length)
                            },
                            _ => panic!("Not an boundary entry route"),
                        }
                    },
                    _ => panic!("Unknown route \"{:?}\"", route_name),
                };

                let train_log = Rc::new(RefCell::new(Vec::new()));
                train_logs.push((name.clone(), params.clone(), train_log.clone()));
                let logger = Box::new(move |i| {
                    //println!(" --- {:?}", i);
                    train_log.borrow_mut().push(i);
                });

                let train_id = train_logs.len()-1;
                let driver = Box::new(
                    railway::driver::Driver::new(&mut sim, train_id, activated, node_idx, auth_dist, 
                          *params, logger, timestep));
                sim.start_process(driver);
            }
        }
    }

    sim.run();


    let h = output::history::History {
        inf: inf_log.replace(Vec::new()),
        trains: train_logs.into_iter().map(|(n,p,v)| (n, p, v.replace(Vec::new()))).collect()
    };

    h
}


use std::path::Path;
pub type AppResult<T> = Result<T, failure::Error>;

pub fn read_file(f :&Path) -> AppResult<String> {
  use std::fs::File;
  use std::io::prelude::*;
  use std::io::BufReader;

  let file = File::open(f)?;
  let mut file = BufReader::new(&file);
  let mut contents = String::new();
  file.read_to_string(&mut contents)?;
  Ok(contents)
}

use input::staticinfrastructure;
use input::dispatch;
pub fn get_infrastructure(s :&Path) -> AppResult<(staticinfrastructure::StaticInfrastructure, staticinfrastructure::InfNames<String>)> {
    let contents = read_file(s)?;
    get_infrastructure_string(&contents)
}

pub fn get_infrastructure_string(s :&str) -> AppResult<(staticinfrastructure::StaticInfrastructure, staticinfrastructure::InfNames<String>)> {
    use input::staticinfrastructure_parser::{lexer, parse, model_from_ast};
  let lex = lexer(&mut s.chars())?;
  let stmts = parse(&lex)?;
  let model = model_from_ast(&stmts)?;
  Ok(model)
}

pub fn get_routes(s :&Path, inf :&staticinfrastructure::InfNames<String>) 
    -> AppResult<staticinfrastructure::Routes<String>> {
    use input::route_parser::{parse, lexer};
    let contents = read_file(s)?;
    let lex = lexer(&mut contents.chars())?;
    let rs = parse(&lex, inf)?;
    Ok(rs)
}

pub fn get_dispatch(s :&Path) -> AppResult<dispatch::Dispatch<String>> {
    let contents = read_file(s)?;
    let d = dispatch::parse_dispatch(&contents)?;
    Ok(d)
}
