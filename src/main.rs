mod app;

fn main() {
    env_logger::init();
    pollster::block_on(app::AppWindow::new(1280, 720, "Epic")).run();
}
