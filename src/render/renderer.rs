use std::time::Duration;

pub trait Renderer {
    fn update(&self, dt: &Duration, context: &super::Context);
    fn render(&self, context: &super::Context);
}
