use minisat::{*, unary::*};
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
    last_signal_set_lit :Option<Bool>,
}

impl SignalOptimizer {
    pub fn new(inf :Infrastructure, usages :Box<[Usage]>) -> SignalOptimizer {
        let mut solver = Solver::new();

        use std::iter::once;
        let all_signals = inf.partial_routes.iter()
            .flat_map(|(_,r)| once(r.entry).chain(once(r.exit)))
            .filter(|e| e != &SignalId::Boundary).collect::<HashSet<_>>();
        let active_signals :HashMap<SignalId,Bool>
            = all_signals.into_iter().map(|x| (x, solver.new_lit())).collect();


        let mut s = SignalOptimizer {
            solver,
            active_signals,
            states: (0..usages.len()).map(|_| vec![]).collect(),
            infrastructure: inf,
            usages,
            //current_signals: None,
            last_signal_set_lit: None,
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
        if let Some(prev) = self.last_signal_set_lit.take() {
            self.solver.add_clause(once(!prev));
        }

        // see if we can solve it now
        'next: loop {
            let all_end_state_conditions = self.states.iter()
                .flat_map(|i| i.last().unwrap().trains.clone()) 
                .collect::<HashMap<TrainId, TrainsState>>();

            let assumption = self.solver.and_literal(end_state_condition(&all_end_state_conditions));
            let (n_signals, n_detectors) = 
                if let Ok(model) = self.solver.solve_under_assumptions(vec![assumption]) {
                // the number of states (and the maximal design) works.
                // Now it is time to optimize for the number of signals.


                let n_signals = self.active_signals.iter()
                    .filter(|(s,_)| if let SignalId::ExternalId(_) = s 
                            { true } else { false })
                    .map(|(_,v)| model.value(v)).count();

                let n_detectors = self.active_signals.iter()
                    .filter(|(s,_)| if let SignalId::Anonymous(_) = s 
                            { true } else { false })
                    .map(|(_,v)| model.value(v)).count();

                info!("optimizer first solve successful at n={}, n_sig={}, n_det={}", self.states[0].len(), n_signals, n_detectors);
                (n_signals,n_detectors)
            } else {
                self.add_state();
                continue 'next;
            };

            // TODO end state condition can be fixed here only if we know we won't add more states
            // later.
            self.solver.add_clause(vec![assumption]);


            // try to optimize the number of signals
            // first count the number of signals and detectors and make
            // a truncated unary number containing the relative values

            let signal_cost = self.active_signals.iter()
                .filter(|(s,_)| if let SignalId::ExternalId(_) = s 
                        { true } else { false })
                .map(|(_,v)| Unary::from_bool(*v).mul_const(relative_cost));

            let detector_cost = self.active_signals.iter()
                .filter(|(s,_)| if let SignalId::Anonymous(_) = s 
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
                    println!("In optimize_signals: Solving with mid={}", mid);
                    if let Ok(model) = self.solver.solve_under_assumptions(
                            vec![sum_cost.lte_const(mid as isize)]) {

                        for (i,_) in self.usages.iter().enumerate() {
                            let schedule = mk_schedule(&self.states[i], &model);
                            debug!("Schedule at mid={}:\n{}", mid, 
                                   format_schedule(&schedule));
                        }

                        // sucess, lower hi bound
                        println!("Success l{} m{} h{}, setting h to m", lo, mid, hi);
                        hi = mid;
                        // TODO make this conditional for later increase 
                        // in the number of signals.
                        self.solver.add_clause(vec![sum_cost.lte_const(mid as isize)]);
                    }  else {
                        println!("Failed  l{} m{} h{}, setting l to m+1", lo, mid, hi);
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
                    vec![sum_cost.lte_const(bound as isize)]) {

                let signals : HashMap<SignalId,bool> = self.active_signals.iter()
                    .map(|(sig,val)| (*sig, model.value(val))).collect();
                let this_set_lit = self.solver.new_lit();
                for (sig,v) in self.active_signals.iter() {
                    // this_set_lit implies that v has its current value
                    self.solver.add_clause(vec![!this_set_lit,
                                           if signals[sig] { *v } else { !*v }]);
                }

                self.last_signal_set_lit = Some(this_set_lit);
                return Some(SignalSet { 
                    solver: &mut self.solver,
                    states: &self.states,
                    usages: &self.usages,
                    this_set_lit: this_set_lit,
                    signals: signals });
            } else {
                    //return Err(format!("In optimize_signals: SAT query failed unexpectedly."));
                    println!("In optimize_signals: SAT query failed unexpectedly.");
                    return None;
            };

        }
    }
}

pub struct SignalSet<'a> {
    solver :&'a mut Solver, 
    states :&'a Vec<Vec<State>>,
    usages :&'a [Usage],
    this_set_lit :Bool,
    signals :HashMap<SignalId, bool>
}

impl<'a> SignalSet<'a> {

    pub fn get_signals(&self) -> &HashMap<SignalId, bool> {
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
        while let Ok(model) = self.solver.solve_under_assumptions(vec![self.this_set_lit]) {
            let schedule = mk_schedule(states, &model);
            debug!("disallow schedule: {:?}", schedule);
            disallow_schedule(&mut self.solver, vec![!self.this_set_lit], states, &schedule);
            results.push(schedule);
        }

        results
    }
}

