use std::time::Duration;

pub trait Renderer {
    fn update(&mut self, dt: &Duration, context: &super::Context);
    fn render(&self, context: &super::Context);
}
