use super::AppStateInner;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalView {
    ApplyConfirm,
    DeleteConfirm,
    QueueNotEmptyConfirm,
    DirectoryDeleteConfirm,
    Queue,
    Help,
    ApplyResult,
}

impl AppStateInner {
    pub fn show_view(&mut self, view: ModalView) {
        self.active_modal = Some(view);
    }

    pub fn close_view(&mut self) -> Option<ModalView> {
        self.active_modal.take()
    }

    pub fn hide_view(&mut self, view: ModalView) {
        if self.active_modal == Some(view) {
            self.active_modal = None;
        }
    }

    pub fn has_view(&self, view: ModalView) -> bool {
        self.active_modal == Some(view)
    }
}
