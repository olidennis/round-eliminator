mod bignum;
mod constraint;
mod line;
mod lineset;
mod problem;

use problem::Problem;

fn main() {
    let text = 
"M <Unmatched> <Unmatched> <Unmatched>
<Pointer> <Pointer> <Pointer> <Pointer>

M <Unmatched><Pointer> <Unmatched><Pointer> <Unmatched><Pointer>
<Unmatched> <Unmatched> <Unmatched> <Unmatched>";
    let mut p0 = Problem::from_line_separated_text(text);
    println!("{}",p0.to_text());

    let mut p1 = p0.speedup();
    println!("{}",p1.to_text());

    let mut p2 = p1.speedup();
    println!("{}",p2.to_text());

    let mut p3 = p2.speedup();
    println!("{}",p3.to_text());

    //let mut p4 = p3.speedup();
    //println!("{}",p4.to_text());
}
