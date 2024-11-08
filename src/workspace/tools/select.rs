use crate::GpuDevice;

use super::{super::Workspace, ActionOrigin, Tool};

pub struct SelectTool;

impl Tool for SelectTool {
    fn name(&self) -> &str {
        "Select"
    }
    fn perform_action(
        &mut self,
        workspace: &mut Workspace,
        _gpu: &GpuDevice,
        origin: ActionOrigin,
    ) {
        println!("Selecting");
    }
}
