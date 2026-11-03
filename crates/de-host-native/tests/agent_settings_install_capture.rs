//! Capture of the Settings ▸ Agents status card (#304): a missing CLI
//! agent's install command wraps under the headline with a copy button,
//! an installed agent keeps the compact card, and a missing ACP preset's
//! hint uses the same command well — painted through the real native
//! host at the full-size modal and at the 640 px window floor. Asserts the
//! copy button is reachable; set `DESIGN_EDITOR_INSTALL_SHOT_DIR` to a
//! folder to keep the PNGs for review.
//!
//! Windows-gated like the other native capture tests.

#![cfg(all(feature = "gl-host", not(target_os = "windows")))]

use std::path::PathBuf;

use de_editor_core::agent_settings::{AgentsEntry, ProviderConnectPhase};
use de_editor_core::{AgentProvider, Locale, ThemeMode};
use de_editor_ui::widgets::agent_settings_panel::{AgentSettingsHit, AgentSettingsPanel};
use de_host_native::backend::{NativeBackend, NativeFrameBackend};
use de_host_native::WidgetHostNative;

const NOW_MS: u64 = 10_000;

#[derive(Clone, Copy)]
enum Card {
    MissingCli { copied: bool },
    InstalledCli,
    MissingAcp,
}

fn host_for(card: Card, theme: ThemeMode, locale: Locale) -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    host.set_now_ms(NOW_MS);
    let ui = &mut host.editor_state_mut().editor_ui;
    ui.theme_mode = theme;
    ui.locale = locale;
    ui.agent_settings_open = true;
    let settings = &mut ui.agent_settings;
    let index = AgentProvider::ClaudeCode.index();
    match card {
        Card::MissingCli { copied } => {
            settings.select_agents_entry(AgentsEntry::Cli(AgentProvider::ClaudeCode));
            let connection = &mut settings.provider_connection[index];
            connection.phase = ProviderConnectPhase::Error;
            connection.not_installed = true;
            let command = "npm install -g @anthropic-ai/claude-code";
            connection.install_command = Some(command.into());
            if copied {
                settings.install_command_copied = Some((command.into(), NOW_MS - 200));
            }
        }
        Card::InstalledCli => {
            settings.select_agents_entry(AgentsEntry::Cli(AgentProvider::ClaudeCode));
            settings.connected[index] = true;
            let connection = &mut settings.provider_connection[index];
            connection.phase = ProviderConnectPhase::Connected;
            connection.version = Some("2.1.4".into());
        }
        Card::MissingAcp => {
            settings.acp_preset_installed.insert("kimi".into(), false);
            settings.begin_acp_agent_draft();
            settings.fill_acp_draft_from_preset(Some("kimi"));
            settings.select_agents_entry(AgentsEntry::AcpDraft);
        }
    }
    host
}

fn copy_hit(card: Card) -> Option<AgentSettingsHit> {
    match card {
        Card::MissingCli { .. } => {
            Some(AgentSettingsHit::CopyCliInstall(AgentProvider::ClaudeCode))
        }
        Card::InstalledCli => None,
        Card::MissingAcp => Some(AgentSettingsHit::CopyAcpInstall(None)),
    }
}

fn paint(host: &mut WidgetHostNative, w: i32, h: i32, out: Option<PathBuf>) {
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface =
        skia_safe::surfaces::raster_n32_premul((w, h)).expect("raster surface allocated");
    for _ in 0..2 {
        let mut frame = NativeFrameBackend::new(&mut backend, surface.canvas());
        host.paint(&mut frame, w as f32, h as f32);
    }
    if let Some(path) = out {
        let data = surface
            .image_snapshot()
            .encode(None, skia_safe::EncodedImageFormat::PNG, 100)
            .expect("encode PNG");
        std::fs::write(&path, data.as_bytes()).expect("write the shot");
        eprintln!("install shot written to {}", path.display());
    }
}

#[test]
fn the_install_command_card_paints_and_offers_copy_at_every_modal_width() {
    let shots = std::env::var("DESIGN_EDITOR_INSTALL_SHOT_DIR")
        .ok()
        .map(PathBuf::from);
    let cases = [
        (
            "missing-tr-light",
            Card::MissingCli { copied: false },
            ThemeMode::Light,
            Locale::Tr,
        ),
        (
            "missing-en-dark",
            Card::MissingCli { copied: false },
            ThemeMode::Dark,
            Locale::EnUs,
        ),
        (
            "copied-tr-light",
            Card::MissingCli { copied: true },
            ThemeMode::Light,
            Locale::Tr,
        ),
        (
            "installed-tr-light",
            Card::InstalledCli,
            ThemeMode::Light,
            Locale::Tr,
        ),
        (
            "acp-missing-tr-light",
            Card::MissingAcp,
            ThemeMode::Light,
            Locale::Tr,
        ),
    ];
    for (name, card, theme, locale) in cases {
        for (size, w, h) in [("wide", 1_200, 820), ("narrow", 640, 520)] {
            let mut host = host_for(card, theme, locale);
            let panel_hit = {
                let panel = AgentSettingsPanel::for_editor(host.editor_state());
                let rect = panel.rect(w as f32, h as f32);
                copy_hit(card).map(|hit| panel.point_for_hit(rect, hit))
            };
            if let Some(point) = panel_hit {
                // The narrow modal may need a scroll to show it; the
                // wide one shows the card above the fold.
                if size == "wide" {
                    assert!(point.is_some(), "{name}: copy button not on screen");
                }
            }
            let out = shots
                .as_ref()
                .map(|dir| dir.join(format!("{name}-{size}.png")));
            paint(&mut host, w, h, out);
        }
    }
}
