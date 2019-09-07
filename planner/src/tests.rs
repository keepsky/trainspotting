use crate::*;

#[test]
fn test_basic() {
    use maplit::*;
    use crate::input::*;

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

    let plan = solver::plan(&Config { n_before: 1, n_after: 0, exact_n :None, optimize_signals: false },
                 &inf, &trains, |_| true).unwrap();

    assert_eq!(plan.len(), 1); // one step
    assert_eq!(plan[0].len(), 1);  // one partial route in the infrastructure
    assert_eq!(plan[0][0], ((0,0), Some(0)));  // the train takes this route
}

#[test]
fn test_overtake() {
    use maplit::*;
    use crate::input::*;

    let inf = Infrastructure {
        partial_routes: hashmap!{
            (0,0) => PartialRoute {
                entry: SignalId::ExternalId(2),
                exit: SignalId::Anonymous(0),
                conflicts: vec![hashset!{((1,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (0,1) => PartialRoute {
                entry: SignalId::Anonymous(0),
                exit: SignalId::ExternalId(0),
                conflicts: vec![hashset!{}],
                wait_conflict: None,
                length: 1000.0,
            },
            (1,0) => PartialRoute {
                entry: SignalId::ExternalId(2),
                exit: SignalId::Anonymous(1),
                conflicts: vec![hashset!{((0,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (1,1) => PartialRoute {
                entry: SignalId::Anonymous(1),
                exit: SignalId::ExternalId(1),
                conflicts: vec![hashset!{}],
                wait_conflict: None,
                length: 1000.0,
            },
            (2,0) => PartialRoute {
                entry: SignalId::ExternalId(0),
                exit: SignalId::Boundary,
                conflicts: vec![hashset!{((3,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (3,0) => PartialRoute {
                entry: SignalId::ExternalId(1),
                exit: SignalId::Boundary,
                conflicts: vec![hashset!{((2,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (4,0) => PartialRoute {
                entry: SignalId::Boundary,
                exit: SignalId::ExternalId(2),
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
                entry: SignalId::ExternalId(2),
                exit: SignalId::Anonymous(0),
                conflicts: vec![hashset!{((1,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (0,1) => PartialRoute {
                entry: SignalId::Anonymous(0),
                exit: SignalId::ExternalId(0),
                conflicts: vec![hashset!{}],
                wait_conflict: None,
                length: 1000.0,
            },
            (1,0) => PartialRoute {
                entry: SignalId::ExternalId(2),
                exit: SignalId::Anonymous(1),
                conflicts: vec![hashset!{((0,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (1,1) => PartialRoute {
                entry: SignalId::Anonymous(1),
                exit: SignalId::ExternalId(1),
                conflicts: vec![hashset!{}],
                wait_conflict: None,
                length: 1000.0,
            },
            (2,0) => PartialRoute {
                entry: SignalId::ExternalId(0),
                exit: SignalId::Boundary,
                conflicts: vec![hashset!{((3,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (3,0) => PartialRoute {
                entry: SignalId::ExternalId(1),
                exit: SignalId::Boundary,
                conflicts: vec![hashset!{((2,0),0)}],
                wait_conflict: None,
                length: 1000.0,
            },
            (4,0) => PartialRoute {
                entry: SignalId::Boundary,
                exit: SignalId::ExternalId(2),
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
