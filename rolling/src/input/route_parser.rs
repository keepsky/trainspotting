use super::staticinfrastructure::*;
use super::parser_utils::*;
use log::*;
//use smallvec::SmallVec;

use std::collections::HashMap;

type Map = HashMap<String, usize>;



pub fn default_release(route :&mut Route) {
    if route.resources.releases.len() > 0 { return; }
    // use last section as trigger
    match route.resources.sections.last() {
        Some(trigger) => {
            let mut resources = Vec::new();
            resources.extend(&route.resources.sections);
            resources.extend(route.resources.switch_positions.iter().map(|&(a,_)| a));
            route.resources.releases.push(Release { 
                end_node: None,
                trigger: *trigger, 
                length: route.length,
                resources: resources.into() });
            //println!("DEFAULT RELEASE: {:?}", route);
        },
        None => warn!("Warning: route has no sections."),
    }
}


// PARSER
//
//


pub fn parse(t: &[Token], inf: &InfNames<String>) -> Result<Routes<String>, ParseError> {
    let mut i = 0;
    let mut routes = HashMap::new();
    while t[i] != Token::EOF {
        if let Some((name, mut route)) = parse_route(&mut i, t, &inf.object_names, &inf.node_names)? {
            default_release(&mut route);
            routes.insert(name, route);
        }
    }
    Ok(routes)
}

fn lookup(names: &HashMap<String, usize>, name: &str) -> Result<usize, ParseError> {
    names.get(name)
        .cloned()
        .ok_or_else(|| ParseError::UnknownName(name.to_string(), "infrastructure".to_string()))
}

pub fn parse_overlaps(i :&mut usize, t: &[Token], objnames :&Map) -> Result<Vec<Overlap>, ParseError> {
    let mut overlaps = Vec::new();
    while matches(i, t, Token::Identifier("overlap".to_string())) {
        let mut name = None;
        while matches(i, t, Token::Named) {
            name = Some(identifier(i,t)?);
        }
        must_match(i, t, Token::BraceOpen)?;

        symbol(i, t, "sections")?;
        let sections = list(i, t, |i, t| lookup(objnames, &identifier(i, t)?))?;
        symbol(i, t, "switches")?;
        let switches = list(i, t, |i, t| {
            //must_match(i, t, Token::ParensOpen)?;
            let sw = lookup(objnames, &identifier(i, t)?)?;
            let pos = alt(i,
                          t,
                          &[&|i, t| {
                                symbol(i, t, "left")?;
                                Ok(SwitchPosition::Left)
                            },
                            &|i, t| {
                                symbol(i, t, "right")?;
                                Ok(SwitchPosition::Right)
                            }])?;
            //must_match(i, t, Token::ParensClose)?;
            Ok((sw, pos))
        })?;

        let mut timeout = None;
        while matches(i, t, Token::Identifier("timeout".to_string())) {
            timeout = Some(number(i,t)?);
        }

        must_match(i, t, Token::BraceClose)?;

        overlaps.push(Overlap { name, sections: sections.into(),
        switch_positions: switches.into(), timeout });
    }

    Ok(overlaps)
}

pub fn parse_resources(i :&mut usize, t: &[Token], objnames :&Map, nodenames: &Map) -> Result<RouteResources, ParseError> {
    symbol(i, t, "sections")?;
    let sections = list(i, t, |i, t| lookup(objnames, &identifier(i, t)?))?;
    symbol(i, t, "switches")?;
    let switches = list(i, t, |i, t| {
        //must_match(i, t, Token::ParensOpen)?;
        let sw = lookup(objnames, &identifier(i, t)?)?;
        let pos = alt(i,
                      t,
                      &[&|i, t| {
                            symbol(i, t, "left")?;
                            Ok(SwitchPosition::Left)
                        },
                        &|i, t| {
                            symbol(i, t, "right")?;
                            Ok(SwitchPosition::Right)
                        }])?;
        //must_match(i, t, Token::ParensClose)?;
        Ok((sw, pos))
    })?;

    symbol(i, t, "contains")?;
    let _contains = list(i, t, |i, t| lookup(nodenames, &identifier(i, t)?))?;

    let mut releases = Vec::new();
    while matches(i, t, Token::Identifier("release".to_string())) {
        must_match(i, t, Token::BraceOpen)?;
        symbol(i, t, "length")?;
        let length = number(i,t)?;
        symbol(i, t, "trigger")?;
        let trigger = lookup(objnames, &identifier(i, t)?)?;
        symbol(i, t, "resources")?;
        let resources = list(i, t, |i, t| lookup(objnames, &identifier(i, t)?))?;
        must_match(i, t, Token::BraceClose)?;
        releases.push(Release {
            end_node: None,
            trigger: trigger,
            length: length,
            resources: resources.into(),
        });
    }

    Ok(RouteResources {
        sections: sections.into(),
        switch_positions: switches.into(),
        releases: releases.into(),
    })
}

pub fn parse_route(i: &mut usize,
                   t: &[Token],
                   objnames: &Map,
                   nodenames: &Map)
                   -> Result<Option<(String, Route)>, ParseError> {
    alt(i,
        t,
        &[&|i, t| {
        symbol(i, t, "modelentry")?;
        let name = identifier(i, t)?;
        symbol(i,t,"from")?;
        let node = lookup(nodenames, &identifier(i,t)?)?;
        must_match(i,t,Token::BraceOpen)?;
        symbol(i,t,"exit")?;
        let exit = lookup(objnames, &identifier(i,t)?)?;
        symbol(i,t,"length")?;
        let length = number(i,t)?;
        let resources = parse_resources(i,t,objnames,nodenames)?;
        must_match(i,t,Token::BraceClose)?;
        Ok(Some((name, Route {
            entry: RouteEntryExit::Boundary(Some(node)),
            exit: RouteEntryExit::Signal(exit),
            length: length,
            resources: resources.into(),
            overlaps: vec![].into(),
            swinging_overlap: false,
        })))
    },
          &|i, t| {
        symbol(i, t, "modelexit")?;
        let name = identifier(i, t)?;
        symbol(i, t, "to")?;
        let node = lookup(nodenames, &identifier(i,t)?).ok();
        must_match(i,t,Token::BraceOpen)?;
        symbol(i,t,"entry")?;
        let entry = lookup(objnames, &identifier(i,t)?)?;
        symbol(i,t,"entrysection")?;
        let entrysection = lookup(objnames, &identifier(i,t)?)?;
        symbol(i,t,"length")?;
        let length = number(i,t)?;
        let resources = parse_resources(i,t,objnames,nodenames)?;
        must_match(i,t,Token::BraceClose)?;
        Ok(Some((name, Route {
            entry: RouteEntryExit::SignalTrigger { signal: entry,
                                         trigger_section: entrysection },
            exit: RouteEntryExit::Boundary(node),
            length: length,
            resources: resources.into(),
            overlaps: vec![].into(),
            swinging_overlap: false,
        })))
    },
          &|i, t| {
        symbol(i, t, "route")?;
        let route_name = identifier(i, t)?;
        must_match(i, t, Token::BraceOpen)?;
        symbol(i, t, "entry")?;
        let entry = lookup(objnames, &identifier(i, t)?)?;
        symbol(i, t, "exit")?;
        let exit = lookup(objnames, &identifier(i, t)?)?;
        // println!("to parse entrysection in route {}", route_name);
        symbol(i, t, "entrysection")?;
        let entrysection = lookup(objnames, &identifier(i, t)?)?;
        symbol(i, t, "length")?;
        let length = number(i, t)?;
        let resources = parse_resources(i,t,objnames,nodenames)?;
        let overlaps = parse_overlaps(i,t,objnames)?;
        let mut swinging = false;
        while matches(i, t, Token::Identifier("swinging".to_string())) {
            swinging = true;
        }
        must_match(i, t, Token::BraceClose)?;
        Ok(Some((route_name,
                 Route {
                     entry: RouteEntryExit::SignalTrigger { signal: entry,
                                                 trigger_section: entrysection },
                     exit: RouteEntryExit::Signal(exit),
                     length: length,
                     resources: resources,
                         overlaps: overlaps.into(),
                         swinging_overlap: swinging,
                 })))
    }])
}

pub fn list<F, O>(i: &mut usize, t: &[Token], f: F) -> Result<Vec<O>, ParseError>
    where F: Fn(&mut usize, &[Token]) -> Result<O, ParseError>
{
    must_match(i, t, Token::ListOpen)?;
    let mut v = Vec::new();
    if matches(i, t, Token::ListClose) {
        return Ok(v);
    }
    loop {
        v.push(f(i, t)?);
        if !matches(i, t, Token::ListSep) {
            break;
        }
    }
    must_match(i, t, Token::ListClose)?;
    Ok(v)
}

pub fn symbol(i: &mut usize, t: &[Token], s: &str) -> Result<(), ParseError> {
    if identifier(i, t)? != s {
        Err(ParseError::UnexpectedToken(*i, format!("{:?}", s)))
    } else {
        Ok(())
    }
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


// LEXER
//
//
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Named,
    BraceOpen,
    BraceClose,
    ListOpen,
    ListClose,
    ListSep,
    ParensOpen,
    ParensClose,
    Number(f64),
    Identifier(String),
    EOF,
}

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
                tokens.push(Token::Identifier(s));
            }
            '#' => {
                input.next().unwrap();
                tokens.push(Token::Named);
            }
            '[' => {
                input.next().unwrap();
                tokens.push(Token::ListOpen);
            }
            ']' => {
                input.next().unwrap();
                tokens.push(Token::ListClose);
            }
            ',' => {
                input.next().unwrap();
                tokens.push(Token::ListSep);
            }
            '(' => {
                input.next().unwrap();
                tokens.push(Token::ParensOpen);
            }
            ')' => {
                input.next().unwrap();
                tokens.push(Token::ParensClose);
            }
            '{' => {
                input.next().unwrap();
                tokens.push(Token::BraceOpen);
            }
            '}' => {
                input.next().unwrap();
                tokens.push(Token::BraceClose);
            }
            ' ' | '\r' | '\t' => {
                input.next().unwrap();
            }
            '\n' => {
                input.next().unwrap();
                line += 1;
            }
            c => {
                error!("Unexpected char \"{}\"",c.escape_debug());
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
