pub mod brush;
pub mod brush_new;
pub mod select;

use crate::GpuDevice;

pub use super::Workspace;
pub use select::SelectTool;

pub trait Tool {
    fn name(&self) -> &str;
    fn perform_action(&mut self, workspace: &mut Workspace, gpu: &GpuDevice, origin: ActionOrigin);
}

impl Default for Box<dyn Tool> {
    fn default() -> Self {
        Box::new(SelectTool)
    }
}

impl Default for &dyn Tool {
    fn default() -> Self {
        &SelectTool
    }
}

pub enum ActionOrigin {
    MouseMove((f32, f32)),
    MouseDown((f32, f32)),
    MouseUp((f32, f32)),
}
