use super::*;

fn run(src: &str) -> Result<String, GlslError> {
    preprocess(lex(src)).map(|tokens| text(&tokens))
}

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn conditionals_keep_the_active_branch() {
    let src = "#if 0\nA\n#elif defined(GL_ES) && __VERSION__ >= 100\nB\n#else\nC\n#endif\n#ifndef MISSING\nD\n#endif\n#ifdef MISSING\nE\n#endif";
    assert_eq!(squash(&run(src).unwrap()), "B D");
}

#[test]
fn nested_conditionals_and_else_branches() {
    let src = "#define LEVEL 2\n#if LEVEL > 1\n#if LEVEL == 3\nthree\n#else\ntwo\n#endif\n#else\none\n#endif";
    assert_eq!(squash(&run(src).unwrap()), "two");
}

#[test]
fn function_like_macros_expand_with_arguments() {
    let src = "#define SQ(x) ((x) * (x))\n#define MIX(a, b) mix(a, b, 0.5)\nfloat v = SQ(1.0 + f(2.0, 3.0)) + MIX(SQ(a), b);";
    assert_eq!(
        squash(&run(src).unwrap()),
        "float v = ((1.0 + f(2.0, 3.0)) * (1.0 + f(2.0, 3.0))) + mix(((a) * (a)), b, 0.5);"
    );
}

#[test]
fn object_macros_follow_c_precedence_and_undef() {
    let src = "#define A 1.0 + 2.0\nx = A * 3.0;\n#undef A\ny = A;";
    assert_eq!(squash(&run(src).unwrap()), "x = 1.0 + 2.0 * 3.0; y = A;");
}

#[test]
fn macros_apply_from_their_definition_on() {
    let src = "a = N;\n#define N 4\nb = N;";
    assert_eq!(squash(&run(src).unwrap()), "a = N; b = 4;");
}

#[test]
fn a_macro_does_not_expand_inside_itself() {
    let src = "#define f(x) f(x + 1)\ny = f(2);";
    assert_eq!(squash(&run(src).unwrap()), "y = f(2 + 1);");
}

#[test]
fn a_function_macro_name_without_call_stays() {
    assert_eq!(
        squash(&run("#define F(x) x\nfloat F;").unwrap()),
        "float F;"
    );
}

#[test]
fn errors_fail_only_when_active() {
    assert!(run("#if 0\n#error nope\n#endif\nok").is_ok());
    assert!(matches!(run("#error nope"), Err(GlslError::Directive(_))));
    assert!(matches!(run("#if 1\nx"), Err(GlslError::Directive(_))));
    assert!(matches!(run("#endif"), Err(GlslError::Directive(_))));
    assert!(matches!(run("#include <x>"), Err(GlslError::Directive(_))));
}

#[test]
fn harmless_directives_and_continuations_are_dropped() {
    let src = "#version 100\n#extension GL_OES_standard_derivatives : enable\n#pragma optimize(on)\n#line 4\n#define LONG 1.0 + \\\n 2.0\nv = LONG;";
    assert_eq!(squash(&run(src).unwrap()), "v = 1.0 + 2.0;");
}

#[test]
fn line_count_is_kept() {
    let out = run("#ifdef GL_ES\nprecision mediump float;\n#endif\nx").unwrap();
    assert_eq!(out.matches('\n').count(), 3, "{out:?}");
}

#[test]
fn expression_evaluator_handles_operators() {
    let defines = HashMap::new();
    for (expr, want) in [
        ("1 + 2 * 3 == 7", true),
        ("(1 << 4) == 0x10", true),
        ("!0 && (5 % 3 == 2)", true),
        ("010 == 8", true),
        ("1 ? 0 : 1", false),
        ("-1 < 0 || 0", true),
        ("UNKNOWN", false),
    ] {
        assert_eq!(evaluate(expr, &defines, &mut 1000), Some(want), "{expr}");
    }
    assert_eq!(evaluate("1 / 0", &defines, &mut 1000), None);
    assert_eq!(evaluate("(1", &defines, &mut 1000), None);
}

#[test]
fn runaway_expansion_fails_instead_of_hanging() {
    let mut src = String::from("#define M0 x x\n");
    for level in 1..40 {
        src.push_str(&format!("#define M{level} M{p} M{p}\n", p = level - 1));
    }
    src.push_str("M39");
    assert_eq!(
        run(&src),
        Err(GlslError::Unsupported("macro expansion too large"))
    );
}

#[test]
fn dropped_branches_keep_their_lines_and_comments_split_words() {
    let out = run("#if 0\na\nb\n#endif\nx").unwrap();
    assert_eq!(out.matches('\n').count(), 4, "{out:?}");
    assert_eq!(squash(&run("#define A/**/B\nA").unwrap()), "B");
    assert_eq!(squash(&run("#define C 1 + \\  \n 2\nC").unwrap()), "1 + 2");
}
