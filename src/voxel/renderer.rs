use std::time::Duration;

use anyhow::Result;

use super::world::WorldManager;
use crate::gfx::Context;

pub trait VoxelRenderer {
    fn update(&mut self, dt: &Duration, context: &Context, world: &mut WorldManager) -> Result<()>;
    fn render(&self, context: &Context) -> Result<()>;
}
