fn main() {
    if let Err(error) = b3_bench::run_cli(std::env::args().skip(1)) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
