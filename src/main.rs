mod core;
mod gfx;
mod math;
mod voxel;

use anyhow::Result;

fn main() -> Result<()> {
    env_logger::init();
    pollster::block_on(core::App::new(1280, 720, "Epic"))?.run()?;
    Ok(())
}
