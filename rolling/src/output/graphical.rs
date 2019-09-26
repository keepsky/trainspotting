use log::*;

use failure::Error;
// use super::history;
// use railway::dynamics::DriverAction;

use std::collections::HashMap;
use input::staticinfrastructure::{StaticInfrastructure, SwitchPosition, Edges, StaticObject, InfNames};
fn get(x :&HashMap<String,usize>, n :usize) -> &str {
    for (k,v) in x.iter() {
        if *v == n { return k; }
    }
    panic!("unknown node/object {}", n);
}

#[derive(Debug,Copy,Clone, Eq, PartialEq)]
pub enum SwDir {
    Outgoing, Incoming
}

#[derive(Debug,Copy,Clone)]
pub enum GNode {
    StartNode(usize),
    EndNode(usize),
    Linear(usize,usize),
    Sw(SwitchPosition, SwDir, usize, (usize,usize)),
}

#[derive(Debug, Fail)]
enum GraphicalError {
#[fail(display = "no model boundary found")]
NoModelBoundary, 
}


#[derive(Debug,Clone)]
pub enum GSNode {
    StartNode(String),
    EndNode(String),
    Sw(SwitchPosition, SwDir, String, (String,String)),
}


pub fn graphical(inf :&StaticInfrastructure, names :&InfNames<String>) -> Result<String,Error> {
    let boundaries = inf.nodes.iter().enumerate().filter_map(|(i,ref n)| { if let Edges::ModelBoundary = n.edges { return Some(i); } else { return None; }} ).collect::<Vec<_>>();
    let boundary = *boundaries.iter().nth(0).ok_or(GraphicalError::NoModelBoundary)?;
    info!("Selected boundary {:?}", boundary);

    // Selected boundary is now on "down" side of double node.

    let mut nodes = Vec::new();
    let mut visited = Vec::new();
    let mut queue = vec![boundary];
    let mut dists = HashMap::new();
    while queue.len() > 0 {
        let n = queue.pop().unwrap();
        visited.push(n);
        visited.push(inf.nodes[n].other_node);

        // Inspect down direction
        match inf.nodes[n].edges {
            Edges::ModelBoundary => {
                nodes.push((n, GNode::StartNode(inf.nodes[n].other_node)));
                dists.insert((n, inf.nodes[n].other_node), 0.0);
            },
            Edges::Nothing => {
                nodes.push((n, GNode::StartNode(inf.nodes[n].other_node)));
                dists.insert((n, inf.nodes[n].other_node), 0.0);
            }
            Edges::Single(a,d) => {
                // Down direction  a -> n | o
                nodes.push((n, GNode::Linear(a, inf.nodes[n].other_node)));
                dists.insert((n, inf.nodes[n].other_node), 0.0);
                dists.insert((a, n), d);

                let opposite_down = inf.nodes[a].other_node;
                if !visited.contains(&opposite_down) { queue.push(opposite_down); }
            },
            Edges::Switchable(obj) => {
                if let StaticObject::Switch { ref left_link, ref right_link, ref branch_side } = inf.objects[obj] {
                    // Switch in down direction == incoming switch
                    nodes.push((n, GNode::Sw(*branch_side, SwDir::Incoming, inf.nodes[n].other_node,
                                             (left_link.0, right_link.0))));
                    dists.insert((left_link.0, n) ,left_link.1);
                    dists.insert((right_link.0, n) ,right_link.1);

                    let left_down = inf.nodes[left_link.0].other_node;
                    if !visited.contains(&left_down) { queue.push(left_down); }
                    let right_down = inf.nodes[right_link.0].other_node;
                    if !visited.contains(&right_down) { queue.push(right_down); }
                } else {
                    panic!("Not a switch");
                }
            }
        }

        // Inspect up direction
        let upnode = inf.nodes[n].other_node;
        match inf.nodes[upnode].edges {
            Edges::ModelBoundary => {
                nodes.push((upnode, GNode::EndNode(n)));
                dists.insert((n, upnode), 0.0);
            },
            Edges::Nothing => {
                nodes.push((upnode, GNode::EndNode(n)));
                dists.insert((n, upnode), 0.0);
            }
            Edges::Single(a,d) => {
                // Up direction   n | o -> a
                nodes.push((upnode, GNode::Linear(n,a)));
                dists.insert((n, upnode), 0.0);
                dists.insert((upnode, a), d);
                if !visited.contains(&a) { queue.push(a); }
            },
            Edges::Switchable(obj) => {
                if let StaticObject::Switch { ref left_link, ref right_link, ref branch_side } = inf.objects[obj] {
                    // Switch in up direction == outgoing switch
                    nodes.push((upnode, GNode::Sw(*branch_side, SwDir::Outgoing, n,
                                             (left_link.0, right_link.0))));
                    dists.insert((upnode, left_link.0), left_link.1);
                    dists.insert((upnode, right_link.0), right_link.1);

                    let left_down = left_link.0;
                    if !visited.contains(&left_down) { queue.push(left_down); }
                    let right_down = right_link.0;
                    if !visited.contains(&right_down) { queue.push(right_down); }
                } else {
                    panic!("Not a switch");
                }
            }
        }
    }

    info!("DISTS {:?}", dists);

    let majornodes = nodes.iter().enumerate().filter_map(|(i,&(_,ref n))|
            if let GNode::Linear(_,_) = *n { None } else { Some(i.clone()) } ).collect::<Vec<_>>();

    let edges = {

        let find = |start :usize,x :usize| {
                let mut intermediate = vec![start];
                let mut next = x.clone();
                while let Some(&(intermed, GNode::Linear(_,b))) = nodes.iter().find(|&&(i,_)| i == next) {
                    intermediate.push(intermed);
                    next = b.clone();
                }
                intermediate.push(next);
                intermediate
        };

        majornodes.iter().map(|&i| match nodes[i] {
            (start, GNode::StartNode(ref x)) => {
                vec![(find(start,*x),0)]
            },
            (start, GNode::Sw(_, SwDir::Outgoing, ref _n1, (ref n2, ref n3))) => {
                // left (n2) on higher level than right (since this is outgoing switch)
                vec![(find(start,*n2),1),(find(start,*n3),0)]
            },
            (start, GNode::Sw(_, SwDir::Incoming, ref n1, (ref _n2, ref _n3))) => {
                vec![(find(start,*n1),0)]
            },
            _ => vec![]
        }).flat_map(|x| x).collect::<Vec<_>>()
    };


    let g = |i| get(&names.node_names, i);
    let edges2 = edges.iter().map(|&(ref e,ref level)| {
        let e2 = e.iter().zip(e.iter().skip(1)).map(|(a,b)| (g(*a).clone(),g(*b).clone(), dists[&(*a,*b)])).collect::<Vec<_>>();
        (e2,level)
    }).collect::<Vec<_>>();
    info!("EDGES2 {:?}", edges2);

    //let edges = edges.into_iter().map(|mut e| {
    //    let end = g(e.pop().unwrap()).clone();
    //    //let mut i = 0;
    //    //while i < e.len()-1 {
    //    //    if inf.nodes[e[i]].other_node == e[i+1] {
    //    //        e.remove(i+1);
    //    //    }
    //    //    i += 1;
    //    //}
    //    let start = g(e.remove(0)).clone();
    //    let mid = e.chunks(2).map(|x| (g(x[0]).clone(),g(x[1]).clone())).collect::<Vec<_>>();
    //    (start,end,mid)
    //}).collect::<Vec<_>>();
    //info!("EDGES {:?}", edges.iter().map(|x| x.iter().map(|y| g(*y).clone()).collect::<Vec<_>>()).collect::<Vec<_>>());// .iter().map(|x| x.iter().map(|y| get(&inf.node_names,*y) ).collect::<Vec<_>>()).collect::<Vec<_>>());


    // Remove linears
    loop {
        let v = nodes.iter().enumerate().filter_map(|(i,&(_,ref n))| 
            if let GNode::Linear(_,_) = *n { Some(i) } else { None }).nth(0);
        if let Some(i) = v {
            if let (this, GNode::Linear(from,to)) = nodes[i].clone() {
                for &mut (_,ref mut n) in &mut nodes {
                    match *n {
                        GNode::StartNode(ref mut x) => if *x == this { *x = to; },
                        GNode::EndNode(ref mut x) => if *x == this { *x = from; },
                        GNode::Linear(ref mut f, ref mut t) => {
                            if *f == this { *f = from; }
                            if *t == this { *t = to; }
                        },
                        GNode::Sw(ref _pos, ref dir, ref mut n1, (ref mut n2, ref mut n3)) => {
                            if *n1 == this && *dir == SwDir::Outgoing { *n1 = from; }
                            if *n1 == this && *dir == SwDir::Incoming { *n1 = to; }
                            if *n2 == this && *dir == SwDir::Outgoing { *n2 = to; }
                            if *n2 == this && *dir == SwDir::Incoming { *n2 = from; }
                            if *n3 == this && *dir == SwDir::Outgoing { *n3 = to; }
                            if *n3 == this && *dir == SwDir::Incoming { *n3 = from; }
                        }
                    }
                }
            } else {
                panic!("not a linear");
            }

            nodes.remove(i);
        } else {
            break;
        }
    }

    let g = |i| get(&names.node_names, i).to_string();
    let snodes = nodes.into_iter().map(|(i,n)| match n {
        GNode::Linear(_,_) => panic!("Unexpected linear"),
        GNode::StartNode(x) => (g(i), GSNode::StartNode(g(x))),
        GNode::EndNode(x) => (g(i), GSNode::EndNode(g(x))),
        GNode::Sw(pos,dir,a,(b,c)) => (g(i), GSNode::Sw(pos,dir, g(a), (g(b),g(c)) )),
    }).collect::<Vec<_>>();

    let node_output = snodes.into_iter().map(|(i,n)| match n {
        GSNode::StartNode(x) => format!("node {} start {}", i, x),
        GSNode::EndNode(x) => format!("node {} end {}", i, x),
        GSNode::Sw(pos,dir,a,(b,c)) => format!("node {} switch {} {} {} {} {}", 
               i, if pos == SwitchPosition::Left { "left" } else { "right" },
                  if dir == SwDir::Outgoing { "outgoing" } else { "incoming" },
                  a,b,c),
    }).collect::<Vec<_>>();

    let edge_output = edges2.into_iter().map(|(e,level)|  {
        let start = e[0].0;
        let end = e[e.len()-1].1;
        let nodes = e.iter().map(|x| format!("{} {} {}", x.0, x.1, x.2)).collect::<Vec<_>>().join(",");
        format!("edge {} {} level {} ({})", start, end, level, nodes)
    }).collect::<Vec<_>>();

    //Ok(format!("{}\n",node_output.join("\n")))
    Ok(format!("{}\n{}\n",node_output.join("\n"),edge_output.join("\n")))
}
