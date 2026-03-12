//! Browser engine internals for browserd.

pub(crate) mod chromium;

use crate::*;
pub(crate) use chromium::*;

pub(crate) struct ChromiumSessionState {
    pub(crate) browser: Arc<HeadlessBrowser>,
    pub(crate) tabs: HashMap<String, Arc<HeadlessTab>>,
    pub(crate) security_incident: Arc<std::sync::Mutex<Option<String>>>,
    pub(crate) _profile_dir: TempDir,
    pub(crate) _proxy: Option<ChromiumSessionProxy>,
}
