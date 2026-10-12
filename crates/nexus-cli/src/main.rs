fn main() {
    match nexus_cli::dispatch() {
        Ok(true) => {}
        Ok(false) => eprintln!("Use run, eval, serve, or devices. See --help."),
        Err(e) => {
            eprintln!("NexusLab: {e}");
            std::process::exit(1);
        }
    }
}
