#[derive(Debug, Clone, Copy)]
pub enum NavigationRecipe {
    InSpine,
    OutsideSpine,
    LinearNo,
    AfterBody,
    NcxOnly,
}

pub fn navigation_recipe(recipe: NavigationRecipe) -> Vec<u8> {
    let nav_in_spine = !matches!(
        recipe,
        NavigationRecipe::OutsideSpine | NavigationRecipe::NcxOnly
    );
    let include_nav = !matches!(recipe, NavigationRecipe::NcxOnly);
    let linear = if matches!(recipe, NavigationRecipe::LinearNo) {
        "no"
    } else {
        "yes"
    };
    let nav_spine = if nav_in_spine {
        if matches!(recipe, NavigationRecipe::AfterBody) {
            format!(
                "<itemref idref=\"body\" linear=\"yes\"/><itemref idref=\"nav\" linear=\"{linear}\"/>"
            )
        } else {
            format!(
                "<itemref idref=\"nav\" linear=\"{linear}\"/><itemref idref=\"body\" linear=\"yes\"/>"
            )
        }
    } else {
        "<itemref idref=\"body\" linear=\"yes\"/>".to_owned()
    };
    let nav_manifest = if include_nav {
        "<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>"
    } else {
        ""
    };
    let bodymatter_landmark = if matches!(recipe, NavigationRecipe::LinearNo) {
        ""
    } else {
        "<li><a epub:type=\"bodymatter\" href=\"body.xhtml\">Body</a></li>"
    };
    let nav_file = if include_nav {
        let nav = format!(
            r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="body.xhtml">Visible <span>TOC</span></a><ol><li><a href="body.xhtml#detail">Detail</a></li></ol></li></ol></nav><nav epub:type="landmarks"><ol>{bodymatter_landmark}</ol></nav></body></html>"#
        );
        Some(("nav.xhtml", nav.into_bytes()))
    } else {
        None
    };
    let ncx_manifest = if matches!(recipe, NavigationRecipe::NcxOnly) {
        "<item id=\"ncx\" href=\"toc.ncx\" media-type=\"application/x-dtbncx+xml\"/>"
    } else {
        ""
    };
    let package = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Recipe Navigation</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language></metadata><manifest><item id="style" href="style.css" media-type="text/css"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>{nav_manifest}{ncx_manifest}</manifest><spine page-progression-direction="ltr">{nav_spine}</spine></package>"#
    );
    let ncx = br#"<?xml version="1.0"?><ncx><navMap><navPoint><navLabel><text>NCX Visible</text></navLabel><content src="body.xhtml"/></navPoint></navMap></ncx>"#;
    let body = br##"<html class="hltr" xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="style.css"/></head><body><h1 id="detail">Body semantic marker</h1><p><a href="#detail">Self</a> <a href="https://www.google.com/">Google</a></p></body></html>"##;
    super::zip_epub(
        &package,
        &[
            (
                "style.css",
                b"body { writing-mode: horizontal-tb; }".to_vec(),
            ),
            ("body.xhtml", body.to_vec()),
            ("toc.ncx", ncx.to_vec()),
        ],
        nav_file,
    )
}
