extern crate regex;
extern crate rolling;
extern crate railml2dgraph;
#[macro_use] extern crate lalrpop_util;
mod ast;
mod railway;
mod export;
lalrpop_mod!(pub grammar);

use std::path::Path;
use rolling::input::staticinfrastructure::*;
use std::collections::HashMap;

pub use export::to_railml;

pub fn convert_railway(s :&str) -> Result<Vec<railway::Track>, String> {
    let regex = regex::Regex::new(r"//.*").unwrap();
    println!("BEFORE {:?}", s);
    let without_comments = regex.replace_all(s, ""); 
    println!("AFTER {:?}", without_comments);
    let stmts = grammar::MilelangParser::new().parse(&without_comments).map_err(|e| format!("{:?}", e))?;
    println!("Parsed: {:?}", stmts);
    let c = railway::convert(stmts)?;
    Ok(c)
}

pub fn convert_dgraph(s :&str) -> Result<(StaticInfrastructure, HashMap<String,Vec<(String,String)>>), String> {
    let regex = regex::Regex::new(r"//.*").unwrap();
    println!("BEFORE {:?}", s);
    let without_comments = regex.replace_all(s, ""); 
    println!("AFTER {:?}", without_comments);
    let stmts = grammar::MilelangParser::new().parse(&without_comments).map_err(|e| format!("{:?}", e))?;
    println!("Parsed {:?}", stmts);
    let branchingmodel = railway::convert(stmts)?;
    let railml = to_railml(branchingmodel).map_err(|e| format!("{:?}", e))?;
    
    let (doc,ns) = railml2dgraph::get_xml_string(&railml, true)?;
    let (model, detectionsection_edges) = railml2dgraph::convert(&doc, &ns)?;
    let s = railml2dgraph::output::dgraph_string(&model)?;

    // Parse string to dgraph infrastructure model
    let inf = rolling::get_infrastructure_string(&s).map_err(|e| format!("{:?}",e))?;
    // Ok((inf,detectionsection_edges)) // keepsky 
    let inf2 = (inf.0, detectionsection_edges);
    Ok(inf2)
}
