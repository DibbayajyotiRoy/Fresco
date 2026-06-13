fn main() {
    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    let app = fresco::gui::FrescoApplication::new();
    std::process::exit(app.run(&args));
}
