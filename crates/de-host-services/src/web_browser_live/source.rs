//! Where live streams come from: the headless browser's screencasts in the
//! daemon, a fake in the tests. The hub sees only these traits.

use std::sync::Arc;

use de_browser_render::{
    BrowserPool, BrowserRenderError, PageStream, StreamFrame, StreamSettings, Viewport,
};

use super::plan::LiveSpec;

/// Called on the stream's own thread when a frame arrived or it ended.
pub(crate) type Waker = Arc<dyn Fn() + Send + Sync>;

/// One running stream; dropping the last handle stops it.
pub(crate) trait LiveStream: Send + Sync {
    /// The newest frame not taken yet; taking it lets the next one come.
    /// An error means the stream ended.
    fn take_frame(&self) -> Result<Option<StreamFrame>, BrowserRenderError>;
    fn set_viewport(&self, viewport: Viewport) -> Result<(), BrowserRenderError>;
    fn set_settings(&self, settings: StreamSettings) -> Result<(), BrowserRenderError>;
}

/// Opens streams.
pub(crate) trait LiveSource: Send + Sync {
    /// Start streaming `spec`, calling `wake` whenever the stream has a
    /// frame or ended. Blocks while the page loads.
    fn open(&self, spec: &LiveSpec, wake: Waker)
        -> Result<Arc<dyn LiveStream>, BrowserRenderError>;
}

impl LiveStream for PageStream {
    fn take_frame(&self) -> Result<Option<StreamFrame>, BrowserRenderError> {
        PageStream::take_frame(self)
    }

    fn set_viewport(&self, viewport: Viewport) -> Result<(), BrowserRenderError> {
        PageStream::set_viewport(self, viewport)
    }

    fn set_settings(&self, settings: StreamSettings) -> Result<(), BrowserRenderError> {
        PageStream::set_settings(self, settings)
    }
}

/// Screencasts in fresh tabs of `pool`'s browser, which the idle reaper
/// ends once no stream, render or page session holds it.
pub(crate) struct BrowserSource {
    pub pool: BrowserPool,
}

impl LiveSource for BrowserSource {
    fn open(
        &self,
        spec: &LiveSpec,
        wake: Waker,
    ) -> Result<Arc<dyn LiveStream>, BrowserRenderError> {
        de_browser_render::start_idle_reaper();
        let stream = PageStream::open(&self.pool, &spec.url, spec.viewport, spec.settings)?;
        let waker = Arc::clone(&wake);
        stream.set_frame_waker(move || waker());
        // A frame that came before the waker was set.
        wake();
        Ok(Arc::new(stream))
    }
}
