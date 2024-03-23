use std::time::Duration;

use anyhow::Result;

pub trait Renderer {
    fn update(&mut self, dt: &Duration, context: &super::Context) -> Result<()>;
    fn render(&self, context: &super::Context) -> Result<()>;
}
