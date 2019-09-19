mod cli;

use clap::{App as ClApp, AppSettings, Arg, SubCommand};



fn main() {
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
                ),
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
                ),
        )
        .setting(AppSettings::SubcommandRequired)
        .get_matches();

    if let Some(s) = matches.subcommand_matches("server") {
        let addr = s.value_of("bindaddr").unwrap();
        cli::server(addr);
    } else if let Some(f) = matches.subcommand_matches("file") {
        let name = f.value_of("file").unwrap();
        let iter: usize = f.value_of("iter").unwrap().parse().unwrap();
        cli::file(name, iter);
    } else if let Some(f) = matches.subcommand_matches("autolb") {
        let name = f.value_of("file").unwrap();
        let labels: usize = f.value_of("labels").unwrap().parse().unwrap();
        let iter: usize = f.value_of("iter").unwrap().parse().unwrap();
        cli::autolb(name, labels, iter);
    } else if let Some(f) = matches.subcommand_matches("autoub") {
        let name = f.value_of("file").unwrap();
        let labels: usize = f.value_of("labels").unwrap().parse().unwrap();
        let iter: usize = f.value_of("iter").unwrap().parse().unwrap();
        cli::autoub(name, labels, iter);
    }
}
