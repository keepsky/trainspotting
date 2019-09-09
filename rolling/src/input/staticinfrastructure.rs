use smallvec::SmallVec;

use std::collections::HashMap;
use std::hash::Hash;
use std::fmt::Debug;

pub type NameMap<Ref> = HashMap<Ref, usize>;

#[derive(Debug)]
pub struct InfNames<Ref : Hash + Eq + Debug> {
    pub node_names: NameMap<Ref>,
    pub object_names: NameMap<Ref>,
}



// 
// STATIC INFRASTRUCTURE
//
pub type Dist = f64;

pub type NodeId = usize;
pub type ObjectId = usize;

#[derive(Debug)]
pub struct StaticInfrastructure {
    pub nodes: Vec<Node>,
    pub objects: Vec<StaticObject>,
}


#[derive(Debug)]
pub struct Node {
    pub other_node: NodeId,
    pub edges: Edges,
    pub objects: SmallVec<[ObjectId; 2]>,
}

#[derive(Debug)]
pub enum Edges {
    Nothing,
    ModelBoundary,
    Single(NodeId, f64),
    Switchable(ObjectId),
}

#[derive(Debug)]
pub enum StaticObject {
    Sight { distance: f64, signal: ObjectId},
    Signal { has_distant: bool },
    Switch {
        left_link: (NodeId, f64),
        right_link: (NodeId, f64),
        branch_side: SwitchPosition,
    },
    TVDLimit {
        enter: Option<ObjectId>,
        exit: Option<ObjectId>,
    },
    TVDSection,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SwitchPosition {
    Left,
    Right,
}


//
// ROUTES
//

pub type Routes<Ref> = HashMap<Ref, Route>;

#[derive(Debug,Copy,Clone,PartialEq,Eq,Hash,PartialOrd,Ord)]
pub enum RouteEntryExit {
    Boundary(Option<NodeId>),
    Signal(ObjectId),
    SignalTrigger { signal: ObjectId, trigger_section: ObjectId },
}

impl RouteEntryExit {
    pub fn is_boundary(&self) -> bool { match self {
        RouteEntryExit::Boundary(_) => true,
        _ => false,
    } }
}

#[derive(Debug,Clone)]
pub struct Route {
    pub entry: RouteEntryExit,
    pub exit: RouteEntryExit,
    pub length: f64,
    pub resources: RouteResources,
    pub overlaps: SmallVec<[Overlap;2]>,
    pub swinging_overlap: bool,
}

#[derive(Debug,Clone)]
pub struct RouteResources {
    pub sections: SmallVec<[ObjectId; 4]>,
    pub switch_positions: SmallVec<[(ObjectId, SwitchPosition); 2]>,
    pub releases: SmallVec<[Release; 2]>,
}

#[derive(Debug,Clone)]
pub struct Release {
    pub trigger: ObjectId,
    pub length: f64,
    pub resources: SmallVec<[ObjectId; 4]>,
    pub end_node :Option<NodeId>,
}

#[derive(Debug,Clone)]
pub struct Overlap {
    pub name :Option<String>,
    pub sections: SmallVec<[ObjectId; 4]>,
    pub switch_positions: SmallVec<[(ObjectId, SwitchPosition); 2]>,
    pub timeout: Option<f64>,
}
