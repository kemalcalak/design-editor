//! Re-export shim: the browser image-search route's provider ladder moved
//! to `de_image_enrich::net::providers` (pure code motion) so design image
//! slots resolve through one shared ladder; every
//! existing `de_host_services::web_image_search::…` path stays valid.

pub use de_image_enrich::net::providers::*;
