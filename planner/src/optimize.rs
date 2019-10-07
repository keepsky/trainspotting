use minisat::{*, unary::*, symbolic::*};
use std::collections::{HashMap, HashSet};
use log::*;

use crate::input::*;
use crate::solver::*;

pub struct SignalOptimizer {
    solver :Solver,
    active_signals :HashMap<SignalId, Bool>,
    states :Vec<Vec<State>>,
    infrastructure :Infrastructure,
    usages :Box<[Usage]>,
    // current_signals :Option<HashSet<SignalId>>, // 
    last_signal_set_clause :Option<Vec<Bool>>,
    failed_states :usize,
}

impl SignalOptimizer {
    pub fn new(inf :Infrastructure, usages :Box<[Usage]>) -> SignalOptimizer {
        let mut solver = Solver::new();

        use std::iter::once;
        let all_signals = inf.partial_routes.iter()
            .flat_map(|(_,r)| once(r.entry).chain(once(r.exit)))
            .filter(|e| match e {
                // TODO removed detectors here, for now.
                //SignalId::Signal(_) | SignalId::Detector(_) => true,
                SignalId::Signal(_) => true,
                _ => false,
            }).collect::<HashSet<_>>();
        let active_signals :HashMap<SignalId,Bool>
            = all_signals.into_iter().map(|x| (x, solver.new_lit())).collect();


        let mut s = SignalOptimizer {
            solver,
            active_signals,
            states: (0..usages.len()).map(|_| vec![]).collect(),
            infrastructure: inf,
            usages,
            failed_states: 0,
            last_signal_set_clause: None,
        };
        //
        // add the first state
        s.add_state();
        s
    }

    pub fn add_state(&mut self) {
        for (usage_idx,usage) in self.usages.iter().enumerate() {
            let prev_state = self.states[usage_idx].last();
            let new_state = mk_state(&mut self.solver, prev_state, 
                       &self.infrastructure, usage,
                       Some(&self.active_signals));
            self.states.get_mut(usage_idx).unwrap().push(new_state);
        }
    }

    pub fn next_signal_set<'b>(&'b mut self) -> Option<SignalSet<'b>> {
        use std::iter::once;
        // TODO parameter
        let relative_cost :usize = 3;

        // If we have already found a set of signals, exclude that one
        if let Some(prev) = self.last_signal_set_clause.take() {
            info!("Excluding prev {:?}", prev);
            self.solver.add_clause(prev);
        }

        // see if we can solve it now
        'next: loop {
            let all_end_state_conditions = self.states.iter()
                .flat_map(|s| s.last().unwrap().trains.iter().map(|(_,ts)| ts));

            let end_state = self.solver.and_literal(end_state_condition(all_end_state_conditions));
            let (n_signals, n_detectors) = 
                if let Ok(model) = self.solver.solve_under_assumptions(vec![end_state]) {

                self.failed_states = 0;

                // the number of states (and the maximal design) works.
                // Now it is time to optimize for the number of signals.


                let n_signals = self.active_signals.iter()
                    .filter(|(s,_)| if let SignalId::Signal(_) = s 
                            { true } else { false })
                    .map(|(_,v)| model.value(v)).count();

                    // TODO with the separate reduce_detectors
                    // model, we don't need to have choice of detectors here.
                let n_detectors = self.active_signals.iter()
                    .filter(|(s,_)| if let SignalId::Detector(_) = s 
                            { true } else { false })
                    .map(|(_,v)| model.value(v)).count();

                info!("optimizer first solve successful at n={}, n_sig={}, n_det={}", self.states[0].len(), n_signals, n_detectors);
                (n_signals,n_detectors)
            } else {
                self.failed_states += 1;
                if self.failed_states > 3 {
                    info!("No more solutions found.");
                    return None;
                }
                info!("Adding state");
                self.add_state();
                continue 'next;
            };

            // TODO end state condition can be fixed here only if we know we won't add more states
            // later.


            // try to optimize the number of signals
            // first count the number of signals and detectors and make
            // a truncated unary number containing the relative values

            let signal_cost = self.active_signals.iter()
                .filter(|(s,_)| if let SignalId::Signal(_) = s 
                        { true } else { false })
                .map(|(_,v)| Unary::from_bool(*v).mul_const(relative_cost));

            let detector_cost = self.active_signals.iter()
                .filter(|(s,_)| if let SignalId::Detector(_) = s 
                        { true } else { false })
                .map(|(_,v)| Unary::from_bool(*v));


            let init_cost :usize = n_signals*relative_cost + n_detectors;
            let costs = signal_cost.chain(detector_cost).collect::<Vec<Unary>>();
            let sum_cost = Unary::sum_truncate(&mut self.solver, costs, init_cost+1);



            let bound : usize = {
                // optimize
                let (mut lo, mut hi) :(usize,usize) = (0,init_cost);

                'minimize: loop {
                    let mid : usize = (lo + hi)/2;
                    info!("In optimize_signals: Solving with mid={}", mid);
                    if let Ok(model) = self.solver.solve_under_assumptions(
                            vec![end_state, sum_cost.lte_const(mid as isize)]) {

                        for (i,_) in self.usages.iter().enumerate() {
                            let schedule = mk_schedule(&self.states[i], &model);
                            debug!("Schedule at mid={}:\n{}", mid, 
                                   format_schedule(&schedule));
                        }

                        // sucess, lower hi bound
                        info!("Success l{} m{} h{}, setting h to m", lo, mid, hi);
                        hi = mid;
                        // TODO make this conditional for later increase 
                        // in the number of signals.
                        //self.solver.add_clause(vec![sum_cost.lte_const(mid as isize)]);
                    }  else {
                        info!("Failed  l{} m{} h{}, setting l to m+1", lo, mid, hi);
                        lo = mid+1;
                    }

                    if lo >= hi {
                        break 'minimize;
                    }
                };

                lo
            };

                // Get model
            if let Ok(model) = self.solver.solve_under_assumptions(
                    vec![end_state, sum_cost.lte_const(bound as isize)]) {
                
                let signals : HashSet<SignalId> = self.active_signals.iter()
                    .filter_map(|(sig,val)| { if model.value(val) { Some(*sig) } else { None } }).collect();

                let signals_lit = self.active_signals.iter()
                    .map(|(_,val)| if model.value(val) { !*val } else { *val }).collect::<Vec<_>>();


                let this_set_lit = self.solver.new_lit();
                for (sig,v) in self.active_signals.iter() {
                    // this_set_lit implies that v has its current value
                    self.solver.add_clause(vec![!this_set_lit,
                                           if signals.contains(sig) { *v } else { !*v }]);
                }

                self.last_signal_set_clause = Some(signals_lit);
                return Some(SignalSet { 
                    solver: &mut self.solver,
                    end_state: end_state,
                    states: &self.states,
                    infrastructure :&self.infrastructure,
                    usages: &self.usages,
                    this_set_lit: this_set_lit,
                    signals: signals });
            } else {
                    //return Err(format!("In optimize_signals: SAT query failed unexpectedly."));
                    info!("In optimize_signals: SAT query failed unexpectedly.");
                    return None;
            };

        }
    }
}

pub struct SignalSet<'a> {
    solver :&'a mut Solver, 
    end_state :Bool,
    states :&'a Vec<Vec<State>>,
    usages :&'a [Usage],
    infrastructure :&'a Infrastructure,
    this_set_lit :Bool,
    signals :HashSet<SignalId>
}

impl<'a> SignalSet<'a> {

    pub fn get_signals(&self) -> &HashSet<SignalId> {
        &self.signals
    }

    pub fn get_dispatches(&mut self) -> Vec<Vec<RoutePlan>> {
        self.usages.iter().enumerate()
            .map(|(i,_)| self.get_usage_dispatch(&self.states[i])).collect()
    }

    fn get_usage_dispatch(&mut self, states :&[State]) -> Vec<RoutePlan> {
        debug!("getusage dispatch");
        //let usage_lit = self.solver.new_lit();
        let mut results = Vec::new();
        let mut assumptions = vec![self.end_state, self.this_set_lit];
        while let Ok(model) = self.solver.solve_under_assumptions(assumptions.iter().cloned()) {
            let schedule = mk_schedule(states, &model);
            debug!("disallow schedule: {:?}", schedule);
            assumptions.extend(disallow_schedule(vec![!self.this_set_lit], states, &schedule));
            results.push(schedule);
        }

        results
    }

    pub fn reduce_detectors(&mut self, usages_plans :&Vec<Vec<RoutePlan>>) -> HashSet<SignalId> {
        info!("reduce_detectors");
        let mut reduce_solver = Solver::new();

        // Partial route boundary (i.e. detector) activation
        let mut boundary_active : HashMap<SignalId, Bool> = HashMap::new();

        // Build boundary lists
        let mut sig_entry_for :HashMap<SignalId,Vec<PartialRouteId>> = HashMap::new();
        let mut sig_exit_for :HashMap<SignalId,Vec<PartialRouteId>> = HashMap::new();

        for (id,r) in self.infrastructure.partial_routes.iter() {
            boundary_active.entry(r.entry).or_insert_with(|| reduce_solver.new_lit());
            boundary_active.entry(r.exit).or_insert_with(|| reduce_solver.new_lit());
            sig_entry_for.entry(r.entry).or_insert(Vec::new()).push(*id);
            sig_exit_for.entry(r.exit).or_insert(Vec::new()).push(*id);
        }

        for (usage_plans,usage) in usages_plans.iter().zip(self.usages) {
            let n_trains = usage.trains.len();
            for plan in usage_plans.iter() {
                for state in plan.iter() {
                    let fixed_occ :HashMap<PartialRouteId, TrainId> = state.iter()
                        .filter_map(|(a,b)| if let Some(b) = *b { Some((*a,b)) } else { None }).collect();

                    let occ = self.infrastructure.partial_routes.iter().map(|(id,r)| {
                        let val = if let Some(train_id) = fixed_occ.get(id) {
                            Symbolic::new(&mut reduce_solver, vec![Some(*train_id)])
                        } else {
                            Symbolic::new(&mut reduce_solver, 
                                          std::iter::once(None).chain((0..n_trains).map(|x| Some(x))).collect())
                        };
                        (id,val)
                    }).collect::<HashMap<_,_>>();

                    // activate detectors which are needed
                    for (s,_) in boundary_active.iter() {
                        for train in 0..n_trains {
                            for r_before in sig_entry_for.get(s).iter().flat_map(|x| x.iter()) {
                                let mut c = vec![ boundary_active[s], !occ[r_before].has_value(&Some(train)) ];
                                for r_after in sig_exit_for.get(s).iter().flat_map(|x| x.iter()) {
                                    c.push(occ[r_after].has_value(&Some(train)));
                                }
                                reduce_solver.add_clause(c);
                            }
                        }
                    }

                    // exclude conflicting routes
                    // TODO overlaps!
                    for (id,r) in self.infrastructure.partial_routes.iter() {
                        for (first_overlap, conflict_set) in r.conflicts.iter().enumerate() {
                            for (conflict_r,_) in conflict_set.iter() {
                                reduce_solver.add_clause(vec![
                                    occ[id].has_value(&None),
                                    occ[conflict_r].has_value(&None),
                                ]);
                            }
                        }
                    }
                }
            }
        }

        let costs = boundary_active.iter().map(|(_,v)| Unary::from_bool(*v));
        let sum_cost = Unary::sum(&mut reduce_solver, costs.collect());

        let bound : usize = {
            // optimize
            let (mut lo, mut hi) :(usize,usize) = (0,boundary_active.len());

            'minimize: loop {
                let mid : usize = (lo + hi)/2;
                info!("In reduce_detectors: Solving with mid={}", mid);
                if let Ok(model) = reduce_solver.solve_under_assumptions(
                        vec![sum_cost.lte_const(mid as isize)]) {

                    // sucess, lower hi bound
                    info!("Success l{} m{} h{}, setting h to m", lo, mid, hi);
                    hi = mid;
                    // TODO make this conditional for later increase 
                    // in the number of signals.
                    reduce_solver.add_clause(vec![sum_cost.lte_const(mid as isize)]);
                }  else {
                    info!("Failed  l{} m{} h{}, setting l to m+1", lo, mid, hi);
                    lo = mid+1;
                }

                if lo >= hi {
                    break 'minimize;
                }
            };

            lo
        };

        if let Ok(model) = reduce_solver.solve_under_assumptions(
                vec![sum_cost.lte_const(bound as isize)]) {
            let result = boundary_active.iter().filter_map(|(s,v)| {
                if model.value(v) { Some(*s) } else { None }
            }).collect();
            info!("Reduce detectors finished:  {:?}", result);
            result
        } else {
            panic!("reduce_detectors: inconsistent problem formulation");
        }
    }
}

