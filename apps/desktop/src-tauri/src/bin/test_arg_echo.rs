fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    println!("COUNT:{}", args.len());
    for (i, arg) in args.iter().enumerate() {
        println!("ARG[{}]:{}", i, arg);
    }
}
