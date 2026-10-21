//! Home's kind × size contracts reach the preset the kind names.

use super::*;
use de_editor_core::{HomeFamily, HomeSize, TaskDraft};

fn detect(family: HomeFamily, size: HomeSize) -> DesignType {
    let draft = TaskDraft {
        text: "a bakery in Lisbon".into(),
        size,
    };
    let prompt = family.generation_prompt(&draft).expect("a brief");
    detect_design_type(&prompt).type_
}

#[test]
fn a_phone_sized_page_stays_a_page() {
    assert_eq!(
        detect(HomeFamily::LandingPage, HomeSize::Mobile390),
        DesignType::LandingPage
    );
    assert_eq!(
        detect(HomeFamily::Dashboard, HomeSize::Mobile390),
        DesignType::DesktopScreen
    );
}

#[test]
fn every_kind_keeps_its_preset_at_its_default_size() {
    assert_eq!(
        detect(HomeFamily::MobileApp, HomeSize::Mobile390),
        DesignType::MobileScreen
    );
    assert_eq!(
        detect(HomeFamily::LandingPage, HomeSize::Desktop1440),
        DesignType::LandingPage
    );
    assert_eq!(
        detect(HomeFamily::Slides, HomeSize::Slide169),
        DesignType::Slides
    );
}
