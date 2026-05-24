use crate::backend::Backend;

#[derive(Default)]
pub struct WlrootsBackend;

impl Backend for WlrootsBackend {
    fn name(&self) -> &'static str {
        "wlroots"
    }
}
