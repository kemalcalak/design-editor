use crate::{get_skill_by_name, resolve_skills, Phase, ResolveOptions, SkillTrigger};

#[test]
fn shader_effect_teaching_resolves_complete_for_its_triggers() {
    let source = get_skill_by_name("shader-effect").expect("shader-effect registered");
    let SkillTrigger::Keywords(keywords) = &source.meta.trigger else {
        panic!("shader-effect must be keyword-gated");
    };
    for keyword in ["glass", "pixelate", "distortion"] {
        assert!(
            keywords.iter().any(|item| item == keyword),
            "missing `{keyword}` trigger"
        );
    }
    assert!(source.content.contains(r#"effects: [{ type: "shader","#));
    assert!(source.content.contains("uniform shader background;"));

    for prompt in [
        "make the hero card look like refractive glass",
        "pixelate the avatar",
        "add a duotone colour grading to the photo",
    ] {
        let context = resolve_skills(Phase::Generation, prompt, &ResolveOptions::default());
        let resolved = context
            .skills
            .iter()
            .find(|skill| skill.meta.name == "shader-effect")
            .unwrap_or_else(|| panic!("`{prompt}` must resolve shader-effect"));
        assert!(
            !resolved.truncated,
            "shader-effect truncated for `{prompt}`"
        );
        assert_eq!(resolved.content, source.content);
    }
}
