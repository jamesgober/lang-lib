//! Wires the `lang-lib` change-event stream onto a `registry-io`
//! [`SyncRegistry`].
//!
//! Available when the `registry` feature is enabled. The registry is a
//! singleton — `lang-lib` owns it and exposes registration through
//! [`crate::Lang::on_change`]. Callbacks fire inline on the thread that
//! triggered the change (the writer, or the watcher thread for
//! `hot-reload`-driven changes), with sub-microsecond dispatch overhead
//! per `registry-io`'s contract.

use std::sync::{Arc, OnceLock};

use registry_io::SyncRegistry;

use crate::change::LangChangeEvent;

static REGISTRY: OnceLock<Arc<SyncRegistry<LangChangeEvent>>> = OnceLock::new();

pub(crate) fn registry() -> &'static Arc<SyncRegistry<LangChangeEvent>> {
    REGISTRY.get_or_init(|| Arc::new(SyncRegistry::new()))
}

pub(crate) fn emit(event: LangChangeEvent) {
    registry().notify(&event);
}
