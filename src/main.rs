mod app;
mod camera;
mod renderer;
mod texture;

fn main() {
    env_logger::init();
    pollster::block_on(app::App::new(1280, 720, "Epic")).run();
}
