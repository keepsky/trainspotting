use maplit::*;
use crate::*;
use crate::input::*;

fn trivial_model() -> (Infrastructure, Usage) {
    let inf = Infrastructure {
        partial_routes: hashmap!{
            (0,0) => PartialRoute {
                entry: SignalId::Boundary,
                exit: SignalId::Boundary,
                conflicts: vec![hashset!{}],
                wait_conflict: None,
                length: 1000.0,
            },
        },
        elementary_routes: vec![hashset!{ (0,0) }],
    };

    let trains = Usage {
        trains: hashmap!{ 0 => Train { length: 100.0, visits: vec![] } },
        train_ord: vec![]
    };

    (inf,trains)
}

#[test]
fn test_basic() {

    let (inf,trains) = trivial_model();

    let plan = solver::plan(&Config { n_before: 1, n_after: 0, exact_n :None, optimize_signals: false },
                 &inf, &trains, |_| true).unwrap();

    assert_eq!(plan.len(), 1); // one step
    assert_eq!(plan[0].len(), 1);  // one partial route in the infrastructure
    assert_eq!(plan[0][0], ((0,0), Some(0)));  // the train takes this route
}

#[test]
fn too_many_states() {
    // it should be allowed to have empty states at the end of the state list

    use crate::solver::{ mk_state, mk_schedule, end_state_condition, disallow_schedule };

    let (inf,trains) = trivial_model();

    let mut s = minisat::Solver::new();
    let s1 = mk_state(&mut s, None,      &inf, &trains, None);
    let s2 = mk_state(&mut s, Some(&s1), &inf, &trains, None);
    let states = vec![s1,s2];

    let model = s.solve_under_assumptions(end_state_condition(
            states.last().unwrap().trains.iter().map(|(_,ts)| ts))).unwrap();
    let plan = mk_schedule(&states, &model);

    assert_eq!(plan.len(), 2); // two steps
    assert_eq!(plan[0].len(), 1);  // one partial route in the infrastructure
    assert_eq!(plan[0][0], ((0,0), Some(0)));  // the train takes this route
    assert_eq!(plan[1][0], ((0,0), None));  // the second state is just empty

    assert_eq!(plan, vec![vec![((0,0), Some(0))],vec![((0,0), None)]]);

    // Dissallowing using only the first state should eliminate the solution
    s.add_clause(disallow_schedule(vec![], 
                      &states[0..1],
                      &plan[0..1].to_vec()));
    let other_solution = s.solve_under_assumptions(end_state_condition(
            states.last().unwrap().trains.iter().map(|(_,ts)| ts)));
    assert!(other_solution.is_err());

    // Disallowing using both states, as would be the default in plan/optimize,
    // should also work the same.
    s.add_clause(disallow_schedule(vec![], &states, &plan));
    let other_solution = s.solve_under_assumptions(end_state_condition(
            states.last().unwrap().trains.iter().map(|(_,ts)| ts)));
    assert!(other_solution.is_err());

}

#[test]
fn test_overtake() {
    use maplit::*;
    use crate::input::*;

    let inf = Infrastructure {
        partial_routes: hashmap!{
            (0,0) => PartialRoute {
                entry: SignalId::Signal(2),
                exit: SignalId::Detector(0),
                conflicts: vec![hashset!{((1,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (0,1) => PartialRoute {
                entry: SignalId::Detector(0),
                exit: SignalId::Signal(0),
                conflicts: vec![hashset!{}],
                wait_conflict: None,
                length: 1000.0,
            },
            (1,0) => PartialRoute {
                entry: SignalId::Signal(2),
                exit: SignalId::Detector(1),
                conflicts: vec![hashset!{((0,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (1,1) => PartialRoute {
                entry: SignalId::Detector(1),
                exit: SignalId::Signal(1),
                conflicts: vec![hashset!{}],
                wait_conflict: None,
                length: 1000.0,
            },
            (2,0) => PartialRoute {
                entry: SignalId::Signal(0),
                exit: SignalId::Boundary,
                conflicts: vec![hashset!{((3,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (3,0) => PartialRoute {
                entry: SignalId::Signal(1),
                exit: SignalId::Boundary,
                conflicts: vec![hashset!{((2,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (4,0) => PartialRoute {
                entry: SignalId::Boundary,
                exit: SignalId::Signal(2),
                conflicts: vec![hashset!{}],
                wait_conflict: None,
                length: 1000.0,
            },
        },
        elementary_routes: vec![
            hashset!{ (0,0), (0,1) },  // internal 1
            hashset!{ (1,0), (1,1) },  // internal 2
            hashset!{ (2,0) }, // exit 1
            hashset!{ (3,0) }, // exit 2
            hashset!{ (4,0) }, // entry
        ],
    };

    let trains = Usage {
        trains: hashmap!{ 
            0 => Train { length: 100.0, visits: vec![hashset!{4}, hashset!{3}] },
            1 => Train { length: 100.0, visits: vec![hashset!{4}, hashset!{2}] },
        },
        train_ord: vec![
            TrainOrd { a: (0,0), b: (1,0) },
            TrainOrd { a: (1,1), b: (0,1) },
        ]
    };

    let plan = solver::plan(&Config { n_before: 3, n_after: 0, exact_n :None, optimize_signals: false },
                 &inf, &trains, |_| true).unwrap();

    //assert_eq!(plan.len(), 3); // one step
    for (i,mut step) in plan.into_iter().enumerate() {
        step.sort();
        println!("step{}: {:?}", i, step);
    }
}

#[test]
fn test_overtake_optimize() {
    use maplit::*;
    use crate::input::*;

    let inf = Infrastructure {
        partial_routes: hashmap!{
            (0,0) => PartialRoute {
                entry: SignalId::Signal(2),
                exit: SignalId::Detector(0),
                conflicts: vec![hashset!{((1,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (0,1) => PartialRoute {
                entry: SignalId::Detector(0),
                exit: SignalId::Signal(0),
                conflicts: vec![hashset!{}],
                wait_conflict: None,
                length: 1000.0,
            },
            (1,0) => PartialRoute {
                entry: SignalId::Signal(2),
                exit: SignalId::Detector(1),
                conflicts: vec![hashset!{((0,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (1,1) => PartialRoute {
                entry: SignalId::Detector(1),
                exit: SignalId::Signal(1),
                conflicts: vec![hashset!{}],
                wait_conflict: None,
                length: 1000.0,
            },
            (2,0) => PartialRoute {
                entry: SignalId::Signal(0),
                exit: SignalId::Boundary,
                conflicts: vec![hashset!{((3,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (3,0) => PartialRoute {
                entry: SignalId::Signal(1),
                exit: SignalId::Boundary,
                conflicts: vec![hashset!{((2,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (4,0) => PartialRoute {
                entry: SignalId::Boundary,
                exit: SignalId::Signal(2),
                conflicts: vec![hashset!{}],
                wait_conflict: None,
                length: 1000.0,
            },
        },
        elementary_routes: vec![
            hashset!{ (0,0), (0,1) },  // internal 1
            hashset!{ (1,0), (1,1) },  // internal 2
            hashset!{ (2,0) }, // exit 1
            hashset!{ (3,0) }, // exit 2
            hashset!{ (4,0) }, // entry
        ],
    };

    let trains = Usage {
        trains: hashmap!{ 
            0 => Train { length: 100.0, visits: vec![hashset!{4}, hashset!{3}] },
            1 => Train { length: 100.0, visits: vec![hashset!{4}, hashset!{2}] },
        },
        train_ord: vec![
            TrainOrd { a: (0,0), b: (1,0) },
            TrainOrd { a: (1,1), b: (0,1) },
        ]
    };

    let usages = vec![trains];
    let mut opt = optimize::SignalOptimizer::new(inf, usages.into());
    {
        let x1 = opt.next_signal_set();
    }
    {
        let x1 = opt.next_signal_set();
    }
        
    //    plan(&Config { n_before: 3, n_after: 0, exact_n :None, optimize_signals: false },
    //             &inf, &trains, |_| true).unwrap();

    ////assert_eq!(plan.len(), 3); // one step
    //for (i,mut step) in plan.into_iter().enumerate() {
    //    step.sort();
    //    println!("step{}: {:?}", i, step);
    //}
}
