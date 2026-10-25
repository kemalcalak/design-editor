//! `PenNode::Browser` → `NodePayload` (#20). The node reaches the scene as
//! `NodeKind::Other("browser")` with its frame, corner radius and stroke;
//! effects ride the shared effect pass. The address and device preset go
//! on the widget descriptor (`value_str` = url, `label` = deviceId) so the
//! canvas painter can label the page box; zoom and scroll stay on the
//! document node, where the page renderer reads them.

use super::*;

pub(super) fn browser_to_payload(n: &BrowserNode) -> NodePayload {
    let mut p = base_payload(&n.base, "browser");
    p.clip_content = true;
    apply_container_style(&mut p, None, n.stroke.as_ref(), n.corner_radius.as_ref());
    p.widget = Some(crate::payload::WidgetPayload {
        kind: "browser".to_string(),
        value_str: n.url.clone().filter(|url| !url.trim().is_empty()),
        label: n.device_id.clone(),
        ..Default::default()
    });
    p
}
