mod core;
mod math;
mod render;
mod voxel;

fn main() {
    env_logger::init();
    pollster::block_on(core::App::new(1280, 720, "Epic")).run();
}
