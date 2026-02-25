use std::process;

fn main() {
    if let Err(error) = cargo_regseq::run() {
        eprintln!("{error}");
        process::exit(1);
    }
}
