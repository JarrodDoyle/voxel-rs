mod app;
mod camera;
mod core;
mod render;

fn main() {
    env_logger::init();
    pollster::block_on(app::App::new(1280, 720, "Epic")).run();
}
