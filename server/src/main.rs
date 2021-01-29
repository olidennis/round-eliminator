mod cli;

use clap::{App as ClApp, AppSettings, Arg, SubCommand};



fn main() {
    env_logger::init();

    let matches = ClApp::new("Sim")
        .version("0.1")
        .about("...")
        .subcommand(
            SubCommand::with_name("file")
                .about("Obtain input from file")
                .arg(
                    Arg::with_name("file")
                        .short("f")
                        .long("file")
                        .value_name("FILE")
                        .required(true)
                        .help("Input file"),
                )
                .arg(
                    Arg::with_name("iter")
                        .short("i")
                        .long("iter")
                        .value_name("ITER")
                        .required(true)
                        .help("Number of iterations"),
                )
                .arg(
                    Arg::with_name("merge")
                        .short("m")
                        .long("merge")
                        .help("Merge equivalent labels after speedup")
                )
                .arg(
                    Arg::with_name("find-periodic-point")
                        .short("p")
                        .long("periodic-point")
                        .required(false)
                        .takes_value(false)
                        .help("Check whether a periodic point has been encountered after each iteration")
                )
        )
        .subcommand(
            SubCommand::with_name("server")
                .about("Spawn an http server")
                .arg(
                    Arg::with_name("bindaddr")
                        .short("a")
                        .long("addr")
                        .default_value("127.0.0.1:8080")
                        .help("bind address"),
                ),
        )
        .subcommand(
            SubCommand::with_name("autolb")
                .about("Find a lower bound automatically using state merging")
                .arg(
                    Arg::with_name("file")
                        .short("f")
                        .long("file")
                        .value_name("FILE")
                        .required(true)
                        .help("Input file"),
                )
                .arg(
                    Arg::with_name("labels")
                        .short("l")
                        .long("labels")
                        .value_name("LABELS")
                        .required(true)
                        .help("Maximum number of labels"),
                )
                .arg(
                    Arg::with_name("iter")
                        .short("i")
                        .long("iter")
                        .value_name("ITER")
                        .required(true)
                        .help("Maximum number of iterations"),
                )
                .arg(
                    Arg::with_name("col")
                        .short("c")
                        .long("col")
                        .value_name("COL")
                        .required(false)
                        .help("size of input coloring"),
                )
                .arg(
                    Arg::with_name("features")
                        .short("x")
                        .long("features")
                        .value_name("FEATURES")
                        .required(false)
                        .help("Simplification types (comma separated). Possible values: unreach (try to merge unreachable labels), addarrow (try to add diagram edges), indirect (try to merge indirect neighbors), diag (try to merge diagram neighbors). Default value: diag,addarrow"),
                ),
        )
        .subcommand(
            SubCommand::with_name("autoub")
                .about("Find an upper bound automatically by removing labels")
                .arg(
                    Arg::with_name("file")
                        .short("f")
                        .long("file")
                        .value_name("FILE")
                        .required(true)
                        .help("Input file"),
                )
                .arg(
                    Arg::with_name("labels")
                        .short("l")
                        .long("labels")
                        .value_name("LABELS")
                        .required(true)
                        .help("Maximum number of labels"),
                )
                .arg(
                    Arg::with_name("iter")
                        .short("i")
                        .long("iter")
                        .value_name("ITER")
                        .required(true)
                        .help("Maximum number of iterations"),
                )
                .arg(
                    Arg::with_name("col")
                        .short("c")
                        .long("col")
                        .value_name("COL")
                        .required(false)
                        .help("size of input coloring (works only in the node-edge case)"),
                )
                .arg(
                    Arg::with_name("features")
                        .short("x")
                        .long("features")
                        .value_name("FEATURES")
                        .required(false)
                        .help("Hardening types (comma separated). Possible values: pred (add predecessors when removing labels), det (some experimental thing). Default value: pred. Note that also `-x \"\"` is allowed."),

                ),
        )
        .setting(AppSettings::SubcommandRequired)
        .get_matches();

    if let Some(s) = matches.subcommand_matches("server") {
        let addr = s.value_of("bindaddr").unwrap();
        cli::server(addr);
    } else if let Some(f) = matches.subcommand_matches("file") {
        let name = f.value_of("file").unwrap();
        let merge = f.is_present("merge");
        let iter: usize = f.value_of("iter").unwrap().parse().unwrap();
        let find_periodic_point: bool = f.is_present("find-periodic-point");
        cli::file(name, iter, merge, find_periodic_point);
    } else if let Some(f) = matches.subcommand_matches("autolb") {
        let name = f.value_of("file").unwrap();
        let labels: usize = f.value_of("labels").unwrap().parse().unwrap();
        let iter: usize = f.value_of("iter").unwrap().parse().unwrap();
        let col : Option<usize> = f.value_of("col").map(|x|x.parse().unwrap());
        let features = f.value_of("features").unwrap_or("diag,addarrow");
        cli::autolb(name, labels, iter,col,features);
    } else if let Some(f) = matches.subcommand_matches("autoub") {
        let name = f.value_of("file").unwrap();
        let labels: usize = f.value_of("labels").unwrap().parse().unwrap();
        let iter: usize = f.value_of("iter").unwrap().parse().unwrap();
        let col : Option<usize> = f.value_of("col").map(|x|x.parse().unwrap());
        let features = f.value_of("features").unwrap_or("pred");
        cli::autoub(name, labels, iter,col,features);
    }
}
