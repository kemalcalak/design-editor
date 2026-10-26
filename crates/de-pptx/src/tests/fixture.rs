//! A small deck written part by part, so every test reads the XML it
//! asserts against. Three slides on one layout / master / theme:
//!
//! 1. title + bulleted body placeholders (positions and styles inherited
//!    from layout and master), speaker notes; stored as `slide9.xml` so
//!    only `sldIdLst` makes it the first slide;
//! 2. one of each shape kind: text box, filled rectangle with text,
//!    gradient ellipse, connector, picture, scaled group, custom path,
//!    table, a rotated + mirrored picture-filled triangle, a numbered
//!    list, a chart with a fallback picture and one without, SmartArt,
//!    an OLE object with its preview, a video;
//! 3. hidden, with its own gradient background.

use std::io::{Cursor, Write};

use base64::Engine;

const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// 1×1 red PNG.
pub const PNG_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

fn ns() -> String {
    format!("xmlns:a=\"{A}\" xmlns:p=\"{P}\" xmlns:r=\"{R}\"")
}

fn rels(entries: &[(&str, &str, &str)]) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
    for (id, kind, target) in entries {
        let mode = if target.starts_with("http") {
            " TargetMode=\"External\""
        } else {
            ""
        };
        out.push_str(&format!(
            "<Relationship Id=\"{id}\" Type=\"{REL}/{kind}\" Target=\"{target}\"{mode}/>"
        ));
    }
    out.push_str("</Relationships>");
    out
}

pub fn xfrm(x: i64, y: i64, cx: i64, cy: i64) -> String {
    format!("<a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>")
}

/// A placeholder shape. `xfrm` empty = inherit the box.
fn ph(id: u32, name: &str, ph: &str, xfrm: &str, body: &str) -> String {
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvSpPr/><p:nvPr>{ph}</p:nvPr></p:nvSpPr>\
<p:spPr>{xfrm}</p:spPr>{body}</p:sp>"
    )
}

fn tx(paragraphs: &str) -> String {
    format!("<p:txBody><a:bodyPr/><a:lstStyle/>{paragraphs}</p:txBody>")
}

fn run(text: &str) -> String {
    format!("<a:p><a:r><a:rPr lang=\"en-US\"/><a:t>{text}</a:t></a:r></a:p>")
}

fn tree(shapes: &str) -> String {
    format!(
        "<p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
<p:grpSpPr/>{shapes}</p:spTree>"
    )
}

fn theme() -> String {
    let slot = |name: &str, hex: &str| format!("<a:{name}><a:srgbClr val=\"{hex}\"/></a:{name}>");
    format!(
        "<a:theme {} name=\"Fixture\"><a:themeElements><a:clrScheme name=\"Fixture\">{}{}{}{}{}{}{}{}{}{}{}{}</a:clrScheme>\
<a:fontScheme name=\"Fixture\"><a:majorFont><a:latin typeface=\"Georgia\"/></a:majorFont>\
<a:minorFont><a:latin typeface=\"Inter\"/></a:minorFont></a:fontScheme><a:fmtScheme name=\"Fixture\"/></a:themeElements></a:theme>",
        ns(),
        "<a:dk1><a:sysClr val=\"windowText\" lastClr=\"000000\"/></a:dk1>",
        slot("lt1", "FFFFFF"),
        slot("dk2", "1F2937"),
        slot("lt2", "E7E6E6"),
        slot("accent1", "4472C4"),
        slot("accent2", "ED7D31"),
        slot("accent3", "A5A5A5"),
        slot("accent4", "FFC000"),
        slot("accent5", "5B9BD5"),
        slot("accent6", "70AD47"),
        slot("hlink", "0563C1"),
        slot("folHlink", "954F72"),
    )
}

fn master() -> String {
    let title = ph(
        2,
        "Title Placeholder",
        "<p:ph type=\"title\"/>",
        &xfrm(838_200, 365_125, 10_515_600, 1_325_563),
        &tx(&run("Master title")),
    );
    let body = ph(
        3,
        "Text Placeholder",
        "<p:ph type=\"body\" idx=\"1\"/>",
        &xfrm(838_200, 1_825_625, 10_515_600, 4_351_338),
        &tx(&run("Master body")),
    );
    let bar = format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"4\" name=\"Accent bar\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
<p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:solidFill><a:schemeClr val=\"accent1\"/></a:solidFill>\
<a:ln><a:noFill/></a:ln></p:spPr></p:sp>",
        xfrm(0, 6_705_600, 12_192_000, 152_400)
    );
    format!(
        "<p:sldMaster {}><p:cSld><p:bg><p:bgPr><a:solidFill><a:schemeClr val=\"bg1\"/></a:solidFill></p:bgPr></p:bg>{}</p:cSld>\
<p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" \
accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>\
<p:txStyles><p:titleStyle><a:lvl1pPr algn=\"l\"><a:defRPr sz=\"4400\" b=\"1\"><a:solidFill><a:schemeClr val=\"tx2\"/></a:solidFill>\
<a:latin typeface=\"+mj-lt\"/></a:defRPr></a:lvl1pPr></p:titleStyle>\
<p:bodyStyle><a:lvl1pPr><a:buFont typeface=\"Arial\"/><a:buChar char=\"•\"/><a:defRPr sz=\"2800\"><a:solidFill><a:schemeClr val=\"tx1\"/></a:solidFill>\
<a:latin typeface=\"+mn-lt\"/></a:defRPr></a:lvl1pPr><a:lvl2pPr><a:buFont typeface=\"Wingdings\"/><a:buChar char=\"§\"/>\
<a:defRPr sz=\"2400\"/></a:lvl2pPr></p:bodyStyle><p:otherStyle><a:defPPr><a:defRPr sz=\"1800\"/></a:defPPr></p:otherStyle></p:txStyles></p:sldMaster>",
        ns(),
        tree(&format!("{title}{body}{bar}")),
    )
}

fn layout() -> String {
    let title = ph(2, "Title 1", "<p:ph type=\"title\"/>", "", "");
    // The layout moves the body down; the slide inherits this box.
    let body = ph(
        3,
        "Content 2",
        "<p:ph idx=\"1\"/>",
        &xfrm(838_200, 2_000_000, 10_515_600, 4_000_000),
        "",
    );
    format!(
        "<p:sldLayout {} type=\"titleAndBody\"><p:cSld name=\"Title and Content\">{}</p:cSld>\
<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>",
        ns(),
        tree(&format!("{title}{body}"))
    )
}

fn slide1() -> String {
    let title = ph(
        2,
        "Title 1",
        "<p:ph type=\"title\"/>",
        "",
        &tx(&run("Quarterly review")),
    );
    let body = ph(
        3,
        "Content 2",
        "<p:ph idx=\"1\"/>",
        "",
        &tx(&format!(
            "{}{}<a:p><a:pPr lvl=\"1\"/><a:r><a:rPr lang=\"en-US\" i=\"1\"/><a:t>Detail</a:t></a:r></a:p>",
            run("Revenue up"),
            run("Costs flat"),
        )),
    );
    format!(
        "<p:sld {}><p:cSld>{}</p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>",
        ns(),
        tree(&format!("{title}{body}"))
    )
}

fn sp(id: u32, name: &str, sp_pr: &str, body: &str) -> String {
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{sp_pr}</p:spPr>{body}</p:sp>"
    )
}

fn pic(id: u32, name: &str, xfrm: &str) -> String {
    format!(
        "<p:pic><p:nvPicPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>\
<p:blipFill><a:blip r:embed=\"rIdImg\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>\
<p:spPr>{xfrm}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>"
    )
}

fn chart_frame(id: u32, x: i64) -> String {
    format!(
        "<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id=\"{id}\" name=\"Chart {id}\"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>\
<p:xfrm><a:off x=\"{x}\" y=\"4800000\"/><a:ext cx=\"1905000\" cy=\"1428750\"/></p:xfrm>\
<a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"><c:chart xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" r:id=\"rIdChart\"/></a:graphicData></a:graphic></p:graphicFrame>"
    )
}

fn slide2() -> String {
    let text_box = sp(
        2,
        "TextBox 1",
        &format!("{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/>", xfrm(457_200, 304_800, 4_572_000, 609_600)),
        "<p:txBody><a:bodyPr wrap=\"square\" lIns=\"0\" tIns=\"0\" rIns=\"0\" bIns=\"0\"/><a:lstStyle/>\
<a:p><a:pPr algn=\"ctr\"/><a:r><a:rPr lang=\"en-US\" sz=\"2000\" b=\"1\"><a:solidFill><a:srgbClr val=\"C00000\"/></a:solidFill>\
<a:latin typeface=\"Arial\"/></a:rPr><a:t>Plain </a:t></a:r><a:r><a:rPr lang=\"en-US\" sz=\"2000\" u=\"sng\">\
<a:hlinkClick r:id=\"rIdLink\"/></a:rPr><a:t>link</a:t></a:r></a:p></p:txBody>",
    );
    let card = format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"3\" name=\"Card\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}\
<a:prstGeom prst=\"roundRect\"><a:avLst><a:gd name=\"adj\" fmla=\"val 10000\"/></a:avLst></a:prstGeom></p:spPr>\
<p:style><a:lnRef idx=\"0\"><a:schemeClr val=\"accent1\"/></a:lnRef><a:fillRef idx=\"1\"><a:schemeClr val=\"accent2\"/></a:fillRef>\
<a:effectRef idx=\"0\"><a:schemeClr val=\"accent1\"/></a:effectRef><a:fontRef idx=\"minor\"><a:schemeClr val=\"lt1\"/></a:fontRef></p:style>\
<p:txBody><a:bodyPr anchor=\"ctr\"/><a:lstStyle/>{}</p:txBody></p:sp>",
        xfrm(457_200, 1_219_200, 2_743_200, 1_371_600),
        run("In a card"),
    );
    let ellipse = sp(
        4,
        "Oval",
        &format!(
            "{}<a:prstGeom prst=\"ellipse\"><a:avLst/></a:prstGeom><a:gradFill><a:gsLst>\
<a:gs pos=\"0\"><a:srgbClr val=\"FFFFFF\"/></a:gs><a:gs pos=\"100000\"><a:schemeClr val=\"accent1\"/></a:gs></a:gsLst>\
<a:lin ang=\"5400000\" scaled=\"0\"/></a:gradFill><a:ln w=\"25400\"><a:solidFill><a:srgbClr val=\"000000\"/></a:solidFill><a:prstDash val=\"dash\"/></a:ln>",
            xfrm(3_657_600, 1_219_200, 1_371_600, 1_371_600)
        ),
        "",
    );
    let connector = String::from(
        "<p:cxnSp><p:nvCxnSpPr><p:cNvPr id=\"5\" name=\"Connector\"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr>\
<p:spPr><a:xfrm flipV=\"1\"><a:off x=\"5486400\" y=\"1219200\"/><a:ext cx=\"952500\" cy=\"952500\"/></a:xfrm>\
<a:prstGeom prst=\"straightConnector1\"><a:avLst/></a:prstGeom><a:ln w=\"19050\"><a:solidFill><a:srgbClr val=\"333333\"/></a:solidFill></a:ln></p:spPr></p:cxnSp>"
    );
    let picture = pic(6, "Logo", &xfrm(6_858_000, 1_219_200, 952_500, 952_500));
    // Child space is twice the group's size: children shrink by half.
    let group = format!(
        "<p:grpSp><p:nvGrpSpPr><p:cNvPr id=\"7\" name=\"Pair\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
<p:grpSpPr><a:xfrm><a:off x=\"8001000\" y=\"1219200\"/><a:ext cx=\"1905000\" cy=\"952500\"/>\
<a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"3810000\" cy=\"1905000\"/></a:xfrm></p:grpSpPr>{}{}</p:grpSp>",
        sp(8, "Left", &format!("{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val=\"00B050\"/></a:solidFill>", xfrm(0, 0, 1_905_000, 1_905_000)), ""),
        sp(9, "Right", &format!("{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val=\"7030A0\"/></a:solidFill>", xfrm(1_905_000, 0, 1_905_000, 1_905_000)), ""),
    );
    let path = sp(
        10,
        "Freeform",
        &format!(
            "{}<a:custGeom><a:pathLst><a:path w=\"100\" h=\"100\"><a:moveTo><a:pt x=\"0\" y=\"50\"/></a:moveTo>\
<a:lnTo><a:pt x=\"50\" y=\"100\"/></a:lnTo><a:lnTo><a:pt x=\"100\" y=\"50\"/></a:lnTo><a:close/></a:path></a:pathLst></a:custGeom>\
<a:solidFill><a:srgbClr val=\"FFC000\"/></a:solidFill>",
            xfrm(457_200, 3_048_000, 952_500, 952_500)
        ),
        "",
    );
    let cell = |text: &str, fill: &str| {
        format!(
            "<a:tc><a:txBody><a:bodyPr/><a:lstStyle/>{}</a:txBody><a:tcPr>{fill}</a:tcPr></a:tc>",
            run(text)
        )
    };
    let table = format!(
        "<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id=\"11\" name=\"Table\"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>\
<p:xfrm><a:off x=\"1905000\" y=\"3048000\"/><a:ext cx=\"3810000\" cy=\"952500\"/></p:xfrm>\
<a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/table\"><a:tbl><a:tblPr/>\
<a:tblGrid><a:gridCol w=\"1905000\"/><a:gridCol w=\"1905000\"/></a:tblGrid>\
<a:tr h=\"476250\">{}{}</a:tr><a:tr h=\"476250\">{}{}</a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>",
        cell("Q1", "<a:solidFill><a:srgbClr val=\"D9E2F3\"/></a:solidFill>"),
        cell("Q2", ""),
        cell("10", ""),
        cell("12", ""),
    );
    let chart_with_fallback = format!(
        "<mc:AlternateContent xmlns:mc=\"{MC}\"><mc:Choice Requires=\"c\">{}</mc:Choice><mc:Fallback>{}</mc:Fallback></mc:AlternateContent>",
        chart_frame(12, 457_200),
        pic(13, "Chart picture", &xfrm(457_200, 4_800_000, 1_905_000, 1_428_750)),
    );
    let bare_chart = chart_frame(14, 3_000_000);
    // Rotated a quarter turn and mirrored, filled with the picture.
    let badge = sp(
        15,
        "Badge",
        &format!(
            "<a:xfrm rot=\"5400000\" flipH=\"1\"><a:off x=\"9144000\" y=\"3048000\"/><a:ext cx=\"952500\" cy=\"952500\"/></a:xfrm>\
<a:prstGeom prst=\"triangle\"><a:avLst/></a:prstGeom><a:blipFill><a:blip r:embed=\"rIdImg\"/><a:stretch><a:fillRect/></a:stretch></a:blipFill>"
        ),
        "",
    );
    let steps = sp(
        16,
        "Steps",
        &format!("{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/>", xfrm(6_096_000, 3_048_000, 2_286_000, 952_500)),
        &tx(&["Plan", "Build", "Ship"]
            .iter()
            .map(|t| format!("<a:p><a:pPr><a:buAutoNum type=\"arabicPeriod\"/></a:pPr><a:r><a:rPr lang=\"en-US\"/><a:t>{t}</a:t></a:r></a:p>"))
            .collect::<String>()),
    );
    let frame = |id: u32, uri: &str, inner: &str| {
        format!(
            "<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id=\"{id}\" name=\"Frame {id}\"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>\
<p:xfrm><a:off x=\"5715000\" y=\"4800000\"/><a:ext cx=\"952500\" cy=\"952500\"/></p:xfrm>\
<a:graphic><a:graphicData uri=\"{uri}\">{inner}</a:graphicData></a:graphic></p:graphicFrame>"
        )
    };
    let smart_art = frame(
        17,
        "http://schemas.openxmlformats.org/drawingml/2006/diagram",
        "",
    );
    let ole = frame(
        18,
        "http://schemas.openxmlformats.org/presentationml/2006/ole",
        &format!(
            "<p:oleObj r:id=\"rIdOle\"><p:embed/>{}</p:oleObj>",
            pic(19, "Object preview", &xfrm(0, 0, 952_500, 952_500))
        ),
    );
    let video = format!(
        "<p:pic><p:nvPicPr><p:cNvPr id=\"20\" name=\"Clip\"/><p:cNvPicPr/><p:nvPr><a:videoFile r:link=\"rIdVideo\"/></p:nvPr></p:nvPicPr>\
<p:blipFill><a:blip r:embed=\"rIdImg\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>\
<p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>",
        xfrm(7_620_000, 4_800_000, 952_500, 952_500)
    );
    format!(
        "<p:sld {}><p:cSld>{}</p:cSld></p:sld>",
        ns(),
        tree(&format!(
            "{text_box}{card}{ellipse}{connector}{picture}{group}{path}{table}{chart_with_fallback}{bare_chart}{badge}{steps}{smart_art}{ole}{video}"
        ))
    )
}

fn slide3() -> String {
    format!(
        "<p:sld {} show=\"0\"><p:cSld><p:bg><p:bgPr><a:gradFill><a:gsLst><a:gs pos=\"0\"><a:srgbClr val=\"000000\"/></a:gs>\
<a:gs pos=\"100000\"><a:srgbClr val=\"333333\"/></a:gs></a:gsLst><a:lin ang=\"0\"/></a:gradFill><a:effectLst/></p:bgPr></p:bg>{}</p:cSld></p:sld>",
        ns(),
        tree(&ph(2, "Title 1", "<p:ph type=\"title\"/>", "", &tx(&run("Backup"))))
    )
}

fn notes1() -> String {
    format!(
        "<p:notes {}><p:cSld>{}</p:cSld></p:notes>",
        ns(),
        tree(&format!(
            "{}{}",
            ph(2, "Slide Image", "<p:ph type=\"sldImg\"/>", "", ""),
            ph(
                3,
                "Notes",
                "<p:ph type=\"body\" idx=\"1\"/>",
                "",
                &tx(&format!(
                    "{}{}",
                    run("Open with the numbers."),
                    run("Then the plan.")
                ))
            )
        ))
    )
}

fn presentation() -> String {
    format!(
        "<p:presentation {}><p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst>\
<p:sldIdLst><p:sldId id=\"256\" r:id=\"rId9\"/><p:sldId id=\"257\" r:id=\"rId3\"/><p:sldId id=\"258\" r:id=\"rId4\"/></p:sldIdLst>\
<p:sldSz cx=\"12192000\" cy=\"6858000\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/>\
<p:defaultTextStyle><a:defPPr><a:defRPr lang=\"en-US\" sz=\"1800\"/></a:defPPr></p:defaultTextStyle></p:presentation>",
        ns()
    )
}

/// The fixture deck as `.pptx` bytes.
pub fn deck() -> Vec<u8> {
    let png = base64::engine::general_purpose::STANDARD
        .decode(PNG_B64)
        .expect("png");
    let parts: Vec<(String, Vec<u8>)> = vec![
        ("[Content_Types].xml".into(), b"<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"/>".to_vec()),
        ("_rels/.rels".into(), rels(&[("rId1", "officeDocument", "ppt/presentation.xml")]).into_bytes()),
        ("ppt/presentation.xml".into(), presentation().into_bytes()),
        (
            "ppt/_rels/presentation.xml.rels".into(),
            rels(&[
                ("rId1", "slideMaster", "slideMasters/slideMaster1.xml"),
                // The title slide is the last part by file name and rel id:
                // only `sldIdLst` puts it first.
                ("rId9", "slide", "slides/slide9.xml"),
                ("rId3", "slide", "slides/slide2.xml"),
                ("rId4", "slide", "slides/slide3.xml"),
            ])
            .into_bytes(),
        ),
        ("ppt/theme/theme1.xml".into(), theme().into_bytes()),
        ("ppt/slideMasters/slideMaster1.xml".into(), master().into_bytes()),
        ("ppt/slideMasters/_rels/slideMaster1.xml.rels".into(), rels(&[("rId1", "theme", "../theme/theme1.xml")]).into_bytes()),
        ("ppt/slideLayouts/slideLayout1.xml".into(), layout().into_bytes()),
        ("ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(), rels(&[("rId1", "slideMaster", "../slideMasters/slideMaster1.xml")]).into_bytes()),
        ("ppt/slides/slide9.xml".into(), slide1().into_bytes()),
        (
            "ppt/slides/_rels/slide9.xml.rels".into(),
            rels(&[("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml"), ("rId2", "notesSlide", "../notesSlides/notesSlide1.xml")]).into_bytes(),
        ),
        ("ppt/slides/slide2.xml".into(), slide2().into_bytes()),
        (
            "ppt/slides/_rels/slide2.xml.rels".into(),
            rels(&[
                ("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml"),
                ("rIdImg", "image", "../media/image1.png"),
                ("rIdLink", "hyperlink", "https://example.com/"),
                ("rIdChart", "chart", "../charts/chart1.xml"),
                ("rIdVideo", "video", "../media/clip.mp4"),
            ])
            .into_bytes(),
        ),
        ("ppt/slides/slide3.xml".into(), slide3().into_bytes()),
        ("ppt/slides/_rels/slide3.xml.rels".into(), rels(&[("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml")]).into_bytes()),
        ("ppt/notesSlides/notesSlide1.xml".into(), notes1().into_bytes()),
        ("ppt/media/image1.png".into(), png),
        ("ppt/media/clip.mp4".into(), vec![0; 64]),
    ];
    zip_parts(&parts)
}

pub fn zip_parts(parts: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in parts {
        writer
            .start_file(name.as_str(), options)
            .expect("zip entry");
        writer.write_all(bytes).expect("zip write");
    }
    writer.finish().expect("zip finish").into_inner()
}
