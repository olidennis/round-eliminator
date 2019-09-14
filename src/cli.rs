use crate::problem::Problem;
use crate::autolb::AutoLb;
use crate::autoub::AutoUb;
use crate::auto::AutomaticSimplifications;

pub fn file(name : &str, iter : usize){
    let data = std::fs::read_to_string(name).expect("Unable to read file");
    let mut p = Problem::from_line_separated_text(&data);
    println!("{}",p.as_result());

    for _ in 0..iter {
        println!("-------------------------");
        p = p.speedup();
        println!("{}",p.as_result());
    }
}

pub fn autolb(name : &str, labels : usize, iter : usize){
    let data = std::fs::read_to_string(name).expect("Unable to read file");
    let p = Problem::from_line_separated_text(&data);
    let auto = AutomaticSimplifications::<AutoLb>::new(p,iter,labels);
    //auto.run(|x|println!("{}",x));
    for x in auto {
        println!("{}",x);
    }
}

pub fn autoub(name : &str, labels : usize, iter : usize){
    let data = std::fs::read_to_string(name).expect("Unable to read file");
    let p = Problem::from_line_separated_text(&data);
    let auto = AutomaticSimplifications::<AutoUb>::new(p,iter,labels);
    //auto.run(|x|println!("{}",x));
    for x in auto {
        println!("{}",x);
    }
}


