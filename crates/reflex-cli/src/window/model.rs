use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowData {
    pub id: String,
    pub title: String,
    pub app_id: Option<String>,
    pub exists: bool,
}

#[derive(Debug, Clone)]
pub struct WindowHandle {
    inner: Arc<RwLock<WindowData>>,
}

impl WindowHandle {
    pub(crate) fn new(id: String, title: String, app_id: Option<String>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(WindowData {
                id,
                title,
                app_id,
                exists: true,
            })),
        }
    }

    pub fn id(&self) -> String {
        self.data().id
    }

    pub fn title(&self) -> String {
        self.data().title
    }

    pub fn app_id(&self) -> Option<String> {
        self.data().app_id
    }

    pub fn exists(&self) -> bool {
        self.data().exists
    }

    pub fn data(&self) -> WindowData {
        self.inner
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(crate) fn update(&self, title: String, app_id: Option<String>) -> WindowData {
        let mut data = self
            .inner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = data.clone();
        data.title = title;
        data.app_id = app_id;
        data.exists = true;
        previous
    }

    pub(crate) fn close(&self) {
        self.inner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .exists = false;
    }
}

#[derive(Debug, Clone)]
pub enum WindowEvent {
    Opened(WindowHandle),
    Closed(WindowHandle),
    TitleChanged { window: WindowHandle, title: String },
}

#[derive(Debug, Clone)]
pub struct WindowSnapshot {
    pub windows: Vec<WindowHandle>,
    pub generation: u64,
}
