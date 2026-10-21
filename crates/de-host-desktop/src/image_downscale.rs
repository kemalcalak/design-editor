//! Re-export shim: the skia down-scale pass moved to
//! `de_image_enrich::net::downscale` (pure code motion, shared with the
//! mobile image-search session); existing paths stay valid.

pub(crate) use de_image_enrich::net::downscale::*;
