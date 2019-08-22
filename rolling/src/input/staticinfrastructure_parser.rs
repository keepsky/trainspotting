use smallvec::SmallVec;

use super::staticinfrastructure;
use super::staticinfrastructure::{Dist, SwitchPosition, InfNames};

use super::parser_utils::*;

// AST
//
//
type NodeName = String;
#[derive(Debug)]
pub enum Statement {
    DoubleNode(PartNode, PartNode),
    Boundary(NodeName),
    Linear(NodeName, NodeName, Dist),
    Switch(String, SwitchPosition, NodeName, Vec<(NodeName, Dist)>),
}

#[derive(Debug)]
pub struct PartNode {
    name: String,
    contents: Vec<Object>,
}

#[derive(Debug)]
pub enum Object {
    Sight(String, Dist),
    Signal(String),
    Exit(String),
    Enter(String),
}

// LEXER
//
//
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Node,
    Linear,
    Switch,
    OpenList,
    CloseList,
    Arrow,
    ListSep,
    Signal,
    Exit,
    Enter,
    Sight,
    Boundary,
    Left,
    Right,
    Number(f64),
    Identifier(String),
    EOF,
}


use std::iter::Peekable;
pub fn lexer(x: &mut Iterator<Item = char>) -> Result<Vec<Token>, LexerError> {
    let mut tokens = Vec::new();
    let mut input = x.peekable();
    let mut line = 0;
    while let Some(&ch) = input.peek() {
        match ch {
            x if x.is_numeric() => {
                let num: String = consume_while(&mut input, |a| {
                        a.is_numeric() || a == '-' || a == 'e' || a == 'E' || a == '.'
                    })
                    .into_iter()
                    .collect();
                tokens.push(Token::Number(num.parse::<f64>().unwrap()));
            }
            x if x.is_alphabetic() => {
                let s: String = consume_while(&mut input, |a| a == '_' || a.is_alphanumeric())
                    .into_iter()
                    .collect();
                tokens.push(match s.as_ref() {
                    "node" => Token::Node,
                    "linear" => Token::Linear,
                    "switch" => Token::Switch,
                    "boundary" => Token::Boundary,
                    "signal" => Token::Signal,
                    "sight" => Token::Sight,
                    "exit" => Token::Exit,
                    "enter" => Token::Enter,
                    "left" => Token::Left,
                    "right" => Token::Right,
                    _ => Token::Identifier(s),
                });
            }
            '(' => {
                input.next().unwrap();
                tokens.push(Token::OpenList);
            }
            ')' => {
                input.next().unwrap();
                tokens.push(Token::CloseList);
            }
            ',' => {
                input.next().unwrap();
                tokens.push(Token::ListSep);
            }
            '-' => {
                input.next().unwrap();
                let mut comment = false;
                if let Some(&x) = input.peek() {
                    if x == '-' { 
                        // Comment 
                        consume_while(&mut input, |a| a != '\n');
                        comment = true;
                    }
                }
                if !comment { tokens.push(Token::Arrow); }
            }
            ' ' | '\r' | '\t' => {
                input.next().unwrap();
            }
            '\n' => {
                input.next().unwrap();
                line += 1;
            }
            c => {
                return Err(LexerError::UnexpectedChar {
                    i: line,
                    c: c.to_string(),
                });
            }
        }
    }
    tokens.push(Token::EOF);
    Ok(tokens)
}

fn consume_while<F>(it: &mut Peekable<&mut Iterator<Item = char>>, x: F) -> Vec<char>
    where F: Fn(char) -> bool
{

    let mut v: Vec<char> = vec![];

    while let Some(&ch) = it.peek() {
        if x(ch) {
            it.next().unwrap();
            v.push(ch);
        } else {
            break;
        }
    }

    v
}


// PARSER
//
//

pub fn parse(t: &[Token]) -> Result<Vec<Statement>, ParseError> {
    let mut i = 0;
    let mut statements = Vec::new();
    while t[i] != Token::EOF {
        statements.push(parse_statement(&mut i, t)?);
    }
    Ok(statements)
}

pub fn identifier(i: &mut usize, tokens: &[Token]) -> Result<String, ParseError> {
    let r = match tokens[*i] {
        Token::Identifier(ref s) => s.clone(),
        ref x => return Err(ParseError::UnexpectedToken(*i, format!("{:?}", x.clone()))),
    };
    *i += 1;
    Ok(r)
}

pub fn number(i: &mut usize, tokens: &[Token]) -> Result<f64, ParseError> {
    let r = match tokens[*i] {
        Token::Number(x) => x,
        ref x => return Err(ParseError::UnexpectedToken(*i, format!("{:?}", x))),
    };
    *i += 1;
    Ok(r)
}


pub fn parse_statement(i: &mut usize, t: &[Token]) -> Result<Statement, ParseError> {
    alt(i,
        t,
        &[&|i, t| {
              must_match(i, t, Token::Boundary)?;
              Ok(Statement::Boundary(identifier(i, t)?))
          },
          &|i, t| {
        must_match(i, t, Token::Node)?;
        let n1 = identifier(i, t)?;
        let n1_objects = parse_opt_object_list(i, t)?;
        must_match(i, t, Token::Arrow)?;
        let n2 = identifier(i, t)?;
        let n2_objects = parse_opt_object_list(i, t)?;
        Ok(Statement::DoubleNode(PartNode {
                                     name: n1,
                                     contents: n1_objects,
                                 },
                                 PartNode {
                                     name: n2,
                                     contents: n2_objects,
                                 }))
    },
          &|i, t| {
        must_match(i, t, Token::Linear)?;
        let n1 = identifier(i, t)?;
        must_match(i, t, Token::Arrow)?;
        let n2 = identifier(i, t)?;
        let dist = number(i, t)?;
        Ok(Statement::Linear(n1, n2, dist))
    },
          &|i, t| {
        must_match(i, t, Token::Switch)?;
        let name = identifier(i, t)?;
        let pos = parse_switchposition(i, t)?;
        let facenode = identifier(i, t)?;
        must_match(i, t, Token::Arrow)?;
        let branches = parse_switch_branches(i, t)?;
        Ok(Statement::Switch(name, pos, facenode, branches))
    }])
}

pub fn parse_switchposition(i :&mut usize, t :&[Token]) 
                            -> Result<SwitchPosition, ParseError> {
    if matches(i,t,Token::Left) {
        return Ok(SwitchPosition::Left);
    }
    if matches(i,t,Token::Right) {
        return Ok(SwitchPosition::Right);
    }
    Err(ParseError::UnexpectedToken(*i, format!("{:?}", t[*i].clone())))
}

pub fn parse_switch_branches(i: &mut usize,
                             t: &[Token])
                             -> Result<Vec<(NodeName, Dist)>, ParseError> {
    let mut branches = Vec::new();
    must_match(i, t, Token::OpenList)?;
    loop {
        let name = identifier(i, t)?;
        let dist = number(i, t)?;
        branches.push((name, dist));
        if !matches(i, t, Token::ListSep) {
            break;
        }
    }
    must_match(i, t, Token::CloseList)?;
    Ok(branches)
}

pub fn parse_opt_object_list(i: &mut usize, t: &[Token]) -> Result<Vec<Object>, ParseError> {
    let mut objs = Vec::new();
    if !matches(i, t, Token::OpenList) {
        return Ok(objs);
    }
    loop {
        objs.push(parse_object(i, t)?);
        if !matches(i, t, Token::ListSep) {
            break;
        }
    }
    must_match(i, t, Token::CloseList)?;
    Ok(objs)
}

pub fn parse_object(i: &mut usize, t: &[Token]) -> Result<Object, ParseError> {
    alt(i,
        t,
        &[&|i, t| {
              must_match(i, t, Token::Sight)?;
              let name = identifier(i, t)?;
              let dist = number(i, t)?;
              Ok(Object::Sight(name, dist))
          },
          &|i, t| {
              must_match(i, t, Token::Signal)?;
              Ok(Object::Signal(identifier(i, t)?))
          },
          &|i, t| {
              must_match(i, t, Token::Exit)?;
              Ok(Object::Exit(identifier(i, t)?))
          },
          &|i, t| {
              must_match(i, t, Token::Enter)?;
              Ok(Object::Enter(identifier(i, t)?))
          }])
}





// CONVERT AST TO MODEL
//
//



#[derive(Debug,Clone,Fail)]
pub enum ModelError {
    #[fail(display = "too many switch legs in {}", _0)]
    SwitchLegs(String),
    #[fail(display = "unknown modeling error")]
    Other,
}

use std::collections::HashMap;
fn id(nodes: &mut HashMap<String, usize>, next: usize, name: &str) -> usize {
    if nodes.contains_key(name) {
        nodes[name]
    } else {
        nodes.insert(name.to_string(), next);
        next
    }
}

fn insert_object(list: &mut Vec<staticinfrastructure::StaticObject>,
                 names: &mut HashMap<String, usize>,
                 obj: staticinfrastructure::StaticObject,
                 name: &str)
                 -> usize {
    if names.contains_key(name) {
        list[names[name]] = obj;
        names[name]
    } else {
        let idx = list.len();
        list.push(obj);
        names.insert(name.to_string(), idx);
        idx
    }
}

fn get_or_create_node(mnodes: &mut Vec<staticinfrastructure::Node>,
                      nodes: &mut HashMap<String, usize>,
                      name: &str)
                      -> usize {
    let next = mnodes.len();
    let idx = id(nodes, next, name);
    if next == idx {
        mnodes.push(staticinfrastructure::Node {
            other_node: 0,
            edges: staticinfrastructure::Edges::Nothing,
            objects: SmallVec::new(),
        });
    }
    idx
}

pub fn model_from_ast(stmts: &[Statement])
                      -> Result<(staticinfrastructure::StaticInfrastructure,staticinfrastructure::InfNames<String>), ModelError> {
    use super::staticinfrastructure::{StaticInfrastructure, Edges};
    let mut model = StaticInfrastructure {
        nodes: Vec::new(),
        objects: Vec::new(),
    };
    let mut names = InfNames {
        node_names: HashMap::new(),
        object_names: HashMap::new(),
    };

    for s in stmts {
        use self::Statement::*;
        match *s {
            Boundary(ref name) => {
                let node_idx = get_or_create_node(&mut model.nodes, &mut names.node_names, name);
                model.nodes[node_idx].edges = Edges::ModelBoundary;
            }
            Linear(ref name1, ref name2, dist) => {
                let n1 = get_or_create_node(&mut model.nodes, &mut names.node_names, name1);
                let n2 = get_or_create_node(&mut model.nodes, &mut names.node_names, name2);
                model.nodes[n1].edges = Edges::Single(n2, dist);
                model.nodes[n2].edges = Edges::Single(n1, dist);
            }
            Switch(ref name, ref side, ref node, ref legs) => {
                let node_idx = get_or_create_node(&mut model.nodes, &mut names.node_names, node);
                if legs.len() != 2 {
                    return Err(ModelError::SwitchLegs(name.clone()));
                }
                let (ref l1name, l1dist) = legs[0];
                let l1idx = get_or_create_node(&mut model.nodes, &mut names.node_names, l1name);
                let (ref l2name, l2dist) = legs[1];
                let l2idx = get_or_create_node(&mut model.nodes, &mut names.node_names, l2name);
                let switch = staticinfrastructure::StaticObject::Switch {
                    left_link: (l1idx, l1dist),
                    right_link: (l2idx, l2dist),
                    branch_side: *side,
                };
                let sw_idx =
                    insert_object(&mut model.objects, &mut names.object_names, switch, name);
                //println!("PARSED switch {:?}", model.objects[sw_idx]);
                model.nodes[node_idx].edges = Edges::Switchable(sw_idx);
                model.nodes[l1idx].edges = Edges::Single(node_idx, l1dist);
                model.nodes[l2idx].edges = Edges::Single(node_idx, l2dist);
            }
            DoubleNode(ref n1, ref n2) => {
                // Create cross references
                //
                let n1_idx = get_or_create_node(&mut model.nodes, &mut names.node_names, &n1.name);
                let n2_idx = get_or_create_node(&mut model.nodes, &mut names.node_names, &n2.name);

                model.nodes[n1_idx].other_node = n2_idx;
                model.nodes[n2_idx].other_node = n1_idx;

                // Create objects and insert references in node
                let mut ins_objs = |objs: &[Object], node| {
                    for obj in objs.iter() {
                        match *obj {
                            Object::Sight(ref name, d) => {
                                let signal = {
                                    let names = &mut names.object_names;
                                    let objs = &mut model.objects;
                                    *names.entry(name.to_string()).or_insert_with(|| {
                                        let idx = objs.len();
                                        objs.push(staticinfrastructure::StaticObject::Signal { has_distant: false });
                                        // TODO allow input of distant signaling capability
                                        idx
                                    })
                                };

                                let object = staticinfrastructure::StaticObject::Sight {
                                    distance: d,
                                    signal: signal,
                                };
                                // let idx = insert_object
                                // (&mut model.objects, &mut objects, object, name);
                                let idx = model.objects.len();
                                model.objects.push(object);
                                let m: &mut staticinfrastructure::Node = &mut model.nodes[node];
                                m.objects.push(idx);
                            }
                            Object::Signal(ref name) => {
                                let idx = insert_object(&mut model.objects,
                                                        &mut names.object_names,
                                                        staticinfrastructure::StaticObject::Signal{
                                                            has_distant: false,
                                                        // TODO allow input of distant signaling capability
                                                        },
                                                        name);
                                let m: &mut staticinfrastructure::Node = &mut model.nodes[node];
                                m.objects.push(idx);
                            }
                            Object::Exit(ref name) => {
                                // Create TVD if necessary
                                let tvd = {
                                    let names = &mut names.object_names;
                                    let objs = &mut model.objects;
                                    *names.entry(name.to_string()).or_insert_with(|| {
                                        let idx = objs.len();
                                        objs.push(staticinfrastructure::StaticObject::TVDSection);
                                        idx
                                    })
                                };

                                let idx = model.objects.len();
                                model.objects.push(staticinfrastructure::StaticObject::TVDLimit {
                                    exit: Some(tvd),
                                    enter: None,
                                });

                                let m: &mut staticinfrastructure::Node = &mut model.nodes[node];
                                m.objects.push(idx);
                            }
                            Object::Enter(ref name) => {
                                // Create TVD if necessary
                                let tvd = {
                                    let names = &mut names.object_names;
                                    let objs = &mut model.objects;
                                    *names.entry(name.to_string()).or_insert_with(|| {
                                        let idx = objs.len();
                                        objs.push(staticinfrastructure::StaticObject::TVDSection);
                                        idx
                                    })
                                };

                                let idx = model.objects.len();
                                model.objects.push(staticinfrastructure::StaticObject::TVDLimit {
                                    enter: Some(tvd),
                                    exit: None,
                                });

                                let m: &mut staticinfrastructure::Node = &mut model.nodes[node];
                                m.objects.push(idx);
                            }
                        }
                    }
                };

                ins_objs(&n1.contents, n1_idx);
                ins_objs(&n2.contents, n2_idx);
            }
        }
    }
    Ok((model,names))
}
