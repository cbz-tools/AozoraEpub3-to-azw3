//! Primary audit coverage: A-04, A-06..A-08, A-12, D-06..D-08,
//! F-06, F-10..F-12.

use crate::support::{NavigationRecipe, convert_epub, navigation_recipe};

// E2E-ID: E2E-NAV-01
// Audit: G2-03, G3-14, G3-15; A-04, A-06..A-08, A-12..A-13, D-06..D-08, F-06, F-10..F-12, F-14
#[test]
fn navigation_sources_preserve_visible_toc_and_reading_order() {
    // Audit coverage: A-04, A-06, A-07, A-08, A-12, A-13; D-06..D-08;
    // F-06, F-10..F-12, F-14 and supported body-link/resource contracts.
    for recipe in [
        NavigationRecipe::InSpine,
        NavigationRecipe::OutsideSpine,
        NavigationRecipe::LinearNo,
        NavigationRecipe::AfterBody,
        NavigationRecipe::NcxOnly,
    ] {
        let azw3 = convert_epub(navigation_recipe(recipe));
        let rawml = String::from_utf8(azw3.rawml()).expect("recipe RawML is UTF-8");
        assert!(
            rawml.contains("Body semantic marker"),
            "body was lost for {recipe:?}"
        );
        assert!(
            rawml.contains("https://www.google.com/"),
            "external link was lost for {recipe:?}"
        );
        assert!(
            rawml.contains("kindle:flow:"),
            "CSS flow was lost for {recipe:?}"
        );
        assert!(
            rawml.contains("kindle:pos:fid:"),
            "body anchor was not reconstructed for {recipe:?}"
        );
        azw3.assert_control_records();
        assert!(azw3.index_report(azw3.mobi().ncx).entry_count >= 1);
        assert!(azw3.index_report(azw3.mobi().guide).entry_count >= 1);

        match recipe {
            NavigationRecipe::NcxOnly => {
                assert_eq!(azw3.ncx_depths(), vec![0], "NCX-only fixture is flat");
                assert!(
                    rawml.contains("NCX Visible"),
                    "NCX-only navigation was not used"
                );
                assert!(!rawml.contains("Visible <span>TOC</span>"));
            }
            NavigationRecipe::OutsideSpine => {
                assert!(
                    !rawml.contains("Visible"),
                    "spine-external nav was incorrectly materialized as visible content"
                );
                assert_eq!(
                    azw3.ncx_depths(),
                    vec![0, 1],
                    "spine-external nav semantics were not parsed as a hierarchy"
                );
            }
            NavigationRecipe::AfterBody => {
                let nav = rawml.find("Visible").expect("visible nav label");
                let body = rawml.find("Body semantic marker").expect("body marker");
                assert!(body < nav, "non-first nav was promoted ahead of body");
                assert_eq!(
                    azw3.ncx_depths(),
                    vec![0, 1],
                    "non-first EPUB nav hierarchy was flattened"
                );
            }
            NavigationRecipe::InSpine | NavigationRecipe::LinearNo => {
                assert_eq!(
                    azw3.ncx_depths(),
                    vec![0, 1],
                    "EPUB nav hierarchy was not serialized into NCX"
                );
                // C-14/C-15 may add list ordinals and span AIDs. Assert the
                // visible label and nested span content independently of
                // serializer attributes.
                assert!(
                    rawml.contains("Visible <span") && rawml.contains(">TOC</span>"),
                    "EPUB nav label/markup was not retained for {recipe:?}"
                );
                assert!(
                    rawml.contains("Detail"),
                    "nested navigation child was not retained for {recipe:?}"
                );
            }
        }
    }
}

// E2E-ID: E2E-NAV-02
// Audit: G2-03; A-04, A-12, B-18, D-08, F-06, F-12
#[test]
fn linear_no_navigation_item_is_retained_without_becoming_bodymatter() {
    // Audit coverage: A-04, A-12, B-18, D-08, F-06, F-12.
    // The source nav is intentionally a non-linear spine item. It remains a
    // visible navigation section, while the Guide body target remains body.
    let azw3 = convert_epub(navigation_recipe(NavigationRecipe::LinearNo));
    let rawml = String::from_utf8(azw3.rawml()).unwrap();
    let nav = rawml.find("Visible").expect("visible nav label");
    let body = rawml.find("Body semantic marker").expect("body marker");
    assert!(nav < body, "linear=no nav was reordered after body");
    let start_reading = azw3.exth().u32(116).expect("Start Reading fallback target") as usize;
    assert!(
        nav < start_reading && start_reading < body,
        "linear=no nav became the Start Reading target"
    );
    let guide = azw3.index_report(azw3.mobi().guide);
    assert!(
        guide.entry_count >= 1,
        "Guide should retain a bodymatter landmark"
    );
}
