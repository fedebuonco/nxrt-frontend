//! The state the commands share: the library, the runs alive now, the last word of each finished run, and the icons read.
use crate::api::ExitInfo;
use crate::library::Library;
use crate::runner::Run;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Serialize, Clone, Default)]
pub struct PadStatus {
    pub connected: bool,
    pub name: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct Shared {
    pub library: Arc<Mutex<Library>>,
    pub runs: Arc<Mutex<HashMap<String, RunHandle>>>,
    pub last: Arc<Mutex<HashMap<String, ExitInfo>>>,
    pub creating: Arc<Mutex<HashMap<String, (PathBuf, PathBuf)>>>,
    pub icons: Arc<Mutex<HashMap<String, Option<String>>>>,
    pub pad: Arc<Mutex<PadStatus>>,
}

impl Shared {
    pub fn new() -> Shared {
        Shared {
            library: Arc::new(Mutex::new(Library::load())),
            runs: Default::default(),
            last: Default::default(),
            creating: Default::default(),
            icons: Default::default(),
            pad: Default::default(),
        }
    }
}

#[derive(Clone)]
pub struct RunHandle {
    pub run: Arc<Mutex<Run>>,
}
