#![feature(is_sorted)]

use std::collections::{HashMap, HashSet};
use algorithms::event::EventHandler;
use constraint::Constraint;
use group::Group;
use itertools::Itertools;
use problem::Problem;
use std::time::Instant;
use crate::algorithms::fixpoint::FixpointType;

pub mod algorithms;
pub mod constraint;
pub mod group;
pub mod line;
pub mod part;
pub mod problem;
pub mod serial;
pub mod directed;
pub mod kpartite;
//#[cfg(test)]
//pub mod moretests;



#[cfg(feature = "all")]
fn fp(problem : &str, hash : &str) -> u128 {
    let problem = std::hint::black_box(problem);
    let eh = &mut EventHandler::null();
    let mut p = Problem::from_string(problem).unwrap();
    p.compute_partial_diagram(eh);

    let start = Instant::now();
    let mut p = p.fixpoint_generic(None,FixpointType::Basic, false, eh).unwrap().0;
    let duration = start.elapsed();

    p.active.lines.sort();
    p.passive.lines.sort();

    //println!("{}",sha256::digest(p.to_string()));
    assert!(sha256::digest(std::hint::black_box(p.to_string())) == hash);

    duration.as_millis()
}

#[cfg(feature = "all")]
fn re(problem : &str, steps : usize, hash : &str) -> u128 {
    let problem = std::hint::black_box(problem);
    let mut eh = &mut EventHandler::null();
    let mut p = Problem::from_string(problem).unwrap();

    let mut r = 0;

    for i in 0..steps-1 {

        p = p.speedup(eh);

        let start = Instant::now();
        p.passive.maximize(&mut eh);
        let duration = start.elapsed();
        if i == steps - 2 {
            r = duration.as_millis();
        }
        
        p.compute_partial_diagram(&mut eh);
        p.sort_active_by_strength();
        p.compute_passive_gen();
        p.rename_by_generators().unwrap();
        p.active.lines.sort();
        p.passive.lines.sort();


    }
    //println!("{}",sha256::digest(p.to_string()));
    assert!(sha256::digest(std::hint::black_box(p.to_string())) == hash);

    r
}

#[cfg(feature = "all")]
pub fn test_all() -> u128 {
    let mut r = fp("A^5 X^2
B^5 Y^2
C^5 Z^2

AX BYCZ
BY CZ
XYZ^2","f20f189c86b1fc3c53b55cb99292e819b49e3e84fc7b775057b447be7d6f5f8d");
    r += re("(0a) (00b) (00c) (00d) (00e)
(0a) (00b) (00c) (00d) (01e)
(0a) (00b) (00c) (01d) (10e)
(0a) (00b) (00c) (01d) (11e)
(0a) (00b) (01c) (10d) (00e)
(0a) (00b) (01c) (10d) (01e)
(0a) (00b) (01c) (11d) (10e)
(0a) (00b) (01c) (11d) (11e)
(0a) (01b) (10c) (00d) (00e)
(0a) (01b) (10c) (00d) (01e)
(0a) (01b) (10c) (01d) (10e)
(0a) (01b) (10c) (01d) (11e)
(0a) (01b) (11c) (10d) (00e)
(0a) (01b) (11c) (10d) (01e)
(0a) (01b) (11c) (11d) (10e)
(0a) (01b) (11c) (11d) (11e)
(1a) (10b) (00c) (00d) (00e)
(1a) (10b) (00c) (00d) (01e)
(1a) (10b) (00c) (01d) (10e)
(1a) (10b) (00c) (01d) (11e)
(1a) (10b) (01c) (10d) (00e)
(1a) (10b) (01c) (10d) (01e)
(1a) (10b) (01c) (11d) (10e)
(1a) (10b) (01c) (11d) (11e)
(1a) (11b) (10c) (00d) (00e)
(1a) (11b) (10c) (00d) (01e)
(1a) (11b) (10c) (01d) (10e)
(1a) (11b) (10c) (01d) (11e)
(1a) (11b) (11c) (10d) (00e)
(1a) (11b) (11c) (10d) (01e)
(1a) (11b) (11c) (11d) (10e)
(1a) (11b) (11c) (11d) (11e)

(0a) (0a) (1a)
(00b) (00b) (00b)
(00b) (01b) (01b)
(00b) (00b) (10b)
(00b) (00b) (11b)
(00b) (01b) (10b)
(00b) (01b) (11b)
(01b) (01b) (10b)
(01b) (01b) (11b)
(00b) (10b) (11b)
(01b) (10b) (10b)
(01b) (11b) (11b)
(10b) (10b) (10b)
(10b) (10b) (11b)
(10b) (11b) (11b)
(11b) (11b) (11b)
(00c) (00c) (00c)
(00c) (01c) (01c)
(00c) (00c) (10c)
(00c) (00c) (11c)
(00c) (01c) (10c)
(00c) (01c) (11c)
(01c) (01c) (10c)
(01c) (01c) (11c)
(00c) (10c) (11c)
(01c) (10c) (10c)
(01c) (11c) (11c)
(10c) (10c) (10c)
(10c) (10c) (11c)
(10c) (11c) (11c)
(11c) (11c) (11c)
(00d) (00d) (00d)
(00d) (01d) (01d)
(00d) (00d) (10d)
(00d) (00d) (11d)
(00d) (01d) (10d)
(00d) (01d) (11d)
(01d) (01d) (10d)
(01d) (01d) (11d)
(00d) (10d) (11d)
(01d) (10d) (10d)
(01d) (11d) (11d)
(10d) (10d) (10d)
(10d) (10d) (11d)
(10d) (11d) (11d)
(11d) (11d) (11d)
(00e) (00e) (00e)
(00e) (01e) (01e)
(00e) (00e) (10e)
(00e) (00e) (11e)
(00e) (01e) (10e)
(00e) (01e) (11e)
(01e) (01e) (10e)
(01e) (01e) (11e)
(00e) (10e) (11e)
(01e) (10e) (10e)
(01e) (11e) (11e)
(10e) (10e) (10e)
(10e) (10e) (11e)
(10e) (11e) (11e)
(11e) (11e) (11e)",2,"74f73f46ec50fe1d9ea196c55b480bcf3ce623f410aa0080904b1c0af84c55f8");
    r
}

#[test]
fn testtest(){
    
    let d = 5;
    let k = 1;

    let letter = |i : usize|{ (b'a' + i as u8) as char };

    for strikeleft in 0..d-k {
        for strikeright in 0..d-k {
            /*if (strikeleft > 0 && strikeright > 0) || strikeright == 1 {
                continue;
            }*/
            if (strikeleft + strikeright >= d-k) || strikeright == 1 {
                continue;
            }
            for choice in (strikeleft..d-k-strikeright).map(|_| [0,1].into_iter() ).multi_cartesian_product() {

                for i in 0..strikeleft {
                    if i == 0 {
                        print!("(-{}) ",letter(i));
                    } else {
                        print!("(--{}) ",letter(i));
                    }
                }

                let mut pred = None;

                for (i,value) in choice.into_iter().enumerate() {
                    let i = i + strikeleft;
                    if let Some(pred) = pred {
                        print!("({}{}{}) ",pred,value,letter(i));
                    } else if i == 0 {
                        print!("({}{}) ",value,letter(i));
                    } else {
                        print!("(-{}{}) ",value,letter(i));
                    }
                    pred = Some(value)
                }

                for i in d-k-strikeright..d-k {
                    if let Some(pred) = pred {
                        print!("({}-{}) ",pred,letter(i));
                    } else {
                        print!("(--{}) ",letter(i));
                    }
                    pred = None;
                }

                for i in d-k..d {
                    print!("(?{}) ",letter(i));
                }
                println!();
            }
        }
    }

    for grab in (0..d).filter(|&i| i != d-k - 1) {
        for i in 0..d {
            if i == grab {
                if i == 0 {
                    print!("(!{}) ",letter(i));
                } else if i < d-k {
                    print!("(!!{}) ",letter(i));
                } else {
                    print!("(!{}) ",letter(i));
                }
            } else if i < d-k {
                if i == 0 {
                    print!("(-{}) ",letter(i));
                } else {
                    print!("(--{}) ",letter(i));
                }
            } else {
                print!("(?{}) ", letter(i));
            }
        }
        println!();
    }


    println!();

    let mut mapping_text_label = HashMap::new();
    let mut passive = Constraint::parse(
"(!!b)(--b)(-0b)(-1b)(0-b)(00b)(01b)(1-b)(10b)(11b) (!!b)(--b)(-0b)(-1b)(0-b)(00b)(01b)(1-b)(10b)(11b) (--b)
(00b) (00b) (00b)
(00b) (01b) (01b)
(00b) (00b) (10b)
(00b) (00b) (11b)
(00b) (01b) (10b)
(00b) (01b) (11b)
(01b) (01b) (10b)
(01b) (01b) (11b)
(00b) (10b) (11b)
(01b) (10b) (10b)
(01b) (11b) (11b)
(10b) (10b) (10b)
(10b) (10b) (11b)
(10b) (11b) (11b)
(11b) (11b) (11b)", &mut mapping_text_label).unwrap();
    let mapping_label_text : HashMap<_,_> = mapping_text_label.iter().map(|(a,b)|(*b,a.clone())).collect();
    passive.maximize(&mut EventHandler::null());

    let passive = passive.edited(|group|{
        let mut g : HashSet<_> = group.iter().cloned().collect();
        let b01 = mapping_text_label["(01b)"];
        let b00 = mapping_text_label["(00b)"];
        let b10 = mapping_text_label["(10b)"];
        let b11 = mapping_text_label["(11b)"];
        let b_0 = mapping_text_label["(-0b)"];
        let b_1 = mapping_text_label["(-1b)"];
        let b0_ = mapping_text_label["(0-b)"];
        let b1_ = mapping_text_label["(1-b)"];
        let b__ = mapping_text_label["(--b)"];
        if g.contains(&b00) && g.contains(&b01) {
            g.insert(b0_);
        }
        if g.contains(&b10) && g.contains(&b11) {
            g.insert(b1_);
        }
        if g.contains(&b00) && g.contains(&b10) {
            g.insert(b_0);
        }
        if g.contains(&b01) && g.contains(&b11) {
            g.insert(b_1);
        }
        g.insert(b__);
        Group::from(g.into_iter().sorted().collect())
    });

    println!("(!a)(-a)(0a)(1a) (-a) (-a)(0a)
(-a)(0a) (-a)(0a) (-a)(1a)");
    for i in 1..d-k {
        for line in &passive.lines {
            let s = line.to_string(&mapping_label_text);
            let s = s.replace(letter(1), &format!("{}",letter(i)));
            println!("{}", s);
        }
    }

    for i in d-k..d {
        let s = "(!d)(?d) (!d)(?d) (?d)";
        let s = s.replace(letter(3), &format!("{}",letter(i)));
        println!("{}", s);

    }

}