use super::{super::Workspace, ActionOrigin, Tool};

pub struct SelectTool;

impl Tool for SelectTool {
    fn name(&self) -> &str {
        "Select"
    }
    fn perform_action(&self, workspace: &mut Workspace, origin: ActionOrigin) {
        println!("Selecting");
    }
}
