use std::collections::{HashMap, HashSet};


// Plan interface

pub struct Config {
    pub n_before: usize,
    pub n_after: usize,
    pub exact_n: Option<usize>,
    pub optimize_signals: bool,
}

pub type RouteId = usize;
pub type PartialRouteId = (RouteId,usize); // Index into Problem.partial_routes
pub type OverlapId = usize; // index into PartialRoute.conflicts
pub type TrainId = usize; // Index into Problem.trains
pub type VisitId = usize; // Index into Train.visits

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
#[derive(Debug)]
pub enum SignalId {
    Boundary,
    ExternalId(usize),
    Anonymous(usize),
}

impl SignalId {
    pub fn is_boundary(&self) -> bool {
        if let SignalId::Boundary = self { true } else { false }
    }
    pub fn is_signal(&self) -> bool {
        if let SignalId::ExternalId(_) = self { true } else { false }
    }

    pub fn is_anonymous(&self) -> bool {
        if let SignalId::Anonymous(_) = self { true } else { false }
    }
}

#[derive(Debug)]
pub struct PartialRoute {
    pub entry: SignalId,
    pub exit: SignalId,
    pub conflicts: Vec<HashSet<(PartialRouteId, usize)>>, // ??
    pub wait_conflict :Option<OverlapId>,
    pub length: f32,
}

type ElementaryRoute = HashSet<PartialRouteId>;

#[derive(Debug)]
pub struct Train {
    pub length: f32,
    pub visits: Vec<HashSet<RouteId>>,
}

#[derive(Debug)]
pub struct TrainOrd {
    pub a :(TrainId, VisitId),
    pub b :(TrainId, VisitId),
}

#[derive(Debug)]
pub struct Infrastructure {
    pub partial_routes: HashMap<PartialRouteId, PartialRoute>,
    pub elementary_routes: Vec<ElementaryRoute>,
}

#[derive(Debug)]
pub struct Usage {
    pub trains: HashMap<TrainId,Train>,
    pub train_ord: Vec<TrainOrd>,
}


// OUTPUT: route plan

pub type RoutePlan = Vec<Vec<(PartialRouteId, Option<TrainId>)>>;

