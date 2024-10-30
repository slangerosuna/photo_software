mod select;

pub use super::Workspace;
pub use select::SelectTool;

pub trait Tool {
    fn name(&self) -> &str;
    fn perform_action(&self, workspace: &mut Workspace, origin: ActionOrigin);
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
    MouseMove,
    MouseDown,
    MouseUp,
}
