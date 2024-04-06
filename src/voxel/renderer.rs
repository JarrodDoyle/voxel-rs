use std::time::Duration;

use anyhow::Result;

use crate::gfx::Context;

pub trait VoxelRenderer {
    fn update(&mut self, dt: &Duration, context: &Context) -> Result<()>;
    fn render(&self, context: &Context) -> Result<()>;
}
