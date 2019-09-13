use crate::problem::Problem;

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
