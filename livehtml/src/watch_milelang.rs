use ViewUpdate;
use std::sync::mpsc::Sender;
use watch;
use serde_json;
use std::path::Path;
use milelang;
use vis_rs;
use rolling::input::staticinfrastructure::*;
use std::collections::HashMap;

fn infrastructure_objects(inf :&StaticInfrastructure, sections: &HashMap<String,Vec<(String,String)>>) -> serde_json::Value {
    fn get(x: &HashMap<String, usize>, n: usize) -> Option<&str> {
        for (k, v) in x.iter() {
            if *v == n {
                return Some(k);
            }
        }
        None
    }

    let mut i = 0;
    let mut j = &mut i;
    let mut fresh = move || { *j += 1; format!("unnamed{}",j) };
    let mut v = json!({});

    // <-- keepsky
    let lookup_node_names = inf.nodes.iter().enumerate().map(|(k,v)| (v.other_node.to_string(), k)).collect::<HashMap<String,usize>>();
    // let lookup_object_names = inf.objects.iter().enumerate().map(|(k,v)| (v.other_node.to_string(), k)).collect::<HashMap<String,usize>>();
    // keepsky -->


    for (node_idx,node) in inf.nodes.iter().enumerate() {
        for obj in &node.objects {
            println!("OBJECT({}) {:?}", obj,inf.objects[*obj]);
            use rolling::input::staticinfrastructure::StaticObject;

            // <-- keepsky
            // let mut data = json!({"node": get(&inf.node_names, node_idx).unwrap()});
            let mut data = json!({"node": get(&lookup_node_names, node_idx).unwrap()});
            // keepsky -->

            match inf.objects[*obj] {
                StaticObject::Signal { .. } => {  // keepsky
                    data.as_object_mut().unwrap().insert(format!("type"),json!("signal"));
                },
                StaticObject::TVDLimit { .. } => {
                    data.as_object_mut().unwrap().insert(format!("type"),json!("detector"));
                },
                StaticObject::Sight { distance, signal } => {
                    data.as_object_mut().unwrap().insert(format!("type"),json!("sight"));
                    data.as_object_mut().unwrap().insert(format!("distance"),json!(distance));
                    // <-- keepsky
                    // data.as_object_mut().unwrap().insert(format!("signal"),json!(get(&inf.object_names, signal).unwrap()));
                    data.as_object_mut().unwrap().insert(format!("signal"),json!(get(&lookup_node_names, signal).unwrap()));
                    // keepsky -->
        },
                StaticObject::Switch { branch_side, .. } => {
                    data.as_object_mut().unwrap().insert(format!("type"),json!("switch"));
                    data.as_object_mut().unwrap().insert(format!("side"),
                    json!(match branch_side {
                        SwitchPosition::Left => "left",
                        SwitchPosition::Right => "right",
                    }));
                },
                _ => { continue; },
            }

            // <-- keepsky
            // v.as_object_mut().unwrap().insert(get(&inf.object_names,*obj).map(|x| x.to_string()).unwrap_or_else(|| fresh()), data);
            v.as_object_mut().unwrap().insert(get(&lookup_node_names,*obj).map(|x| x.to_string()).unwrap_or_else(|| fresh()), data);
            // keepsky -->
        }
    }

    // TVD section objects are not on a node.
    for (i,obj) in inf.objects.iter().enumerate() {
        let mut data = json!({});
        match obj {
            StaticObject::TVDSection => {
                data.as_object_mut().unwrap().insert(format!("type"),json!("tvdsection"));
                // <-- keepsky
                // let name = get(&inf.object_names,i).unwrap();
                let name = get(&lookup_node_names,i).unwrap();
                // keepsky -->

                let edges :Vec<_>= sections[name].iter().map(|(a,b)| format!("{}-{}",a,b)).collect();
                data.as_object_mut().unwrap().insert(format!("edges"), json!(edges));
                // <-- keepsky
                // v.as_object_mut().unwrap().insert(get(&inf.object_names,i).map(|x| x.to_string()).unwrap_or_else(|| fresh()), data);
                v.as_object_mut().unwrap().insert(get(&lookup_node_names,i).map(|x| x.to_string()).unwrap_or_else(|| fresh()), data);
                // keepsky -->
            }
            _ => {},
        }
    }

    v
}

fn schematic_update(s :&str) -> Result<serde_json::Value, String> {
    let (inf,sections) = milelang::convert_dgraph(s).map_err(|e| format!("{:?}",e))?;
    let object_data = infrastructure_objects(&inf, &sections);
    let schematic = vis_rs::convert_dgraph(&inf)?;
    let (edge_lines,node_data) = vis_rs::convert_javascript(schematic)?;
    Ok(json!({"lines": edge_lines, "nodes": node_data, "objects": object_data}))
}

pub fn forever(file :&Path, tx :Sender<ViewUpdate>) {
    watch::update_file_string(file, move |s| {
        println!("Input update.");
        match schematic_update(&s) {
            Ok(json_data) => {
                tx.send(ViewUpdate::Schematic { json_data }).unwrap();
            },
            Err(e) => {
                println!("Input error: {:?}", e);
                // TODO send error message to frontend?
                // tx.send(ViewUpdate::Error(format!("{:?}", e))).unwrap();
            }
        };

    });
}
