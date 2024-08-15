#![feature(is_sorted)]

use std::collections::{HashMap, HashSet};

use algorithms::event::EventHandler;
use constraint::Constraint;
use group::Group;
use itertools::Itertools;
use problem::Problem;
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