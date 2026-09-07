mod support;

use std::collections::HashSet;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use support::{Azw3, SOVEREIGN_EPUB, convert_fixture, epub_text};

#[test]
fn sovereign_stars_preserves_source_semantics_through_kf8() {
    // Audit coverage: A-01, A-03..A-14; applicable B/C/D/E contracts;
    // F-01..F-08 and F-10..F-14. Low-level D geometry is asserted in
    // large_index_e2e; this fixture does not claim item-level page-flow
    // coverage.
    let source = epub_text(SOVEREIGN_EPUB);
    let (_, azw3) = convert_fixture(SOVEREIGN_EPUB);
    let rawml = String::from_utf8(azw3.rawml()).expect("Sovereign RawML is UTF-8");

    // These probes are asserted in both source and output: an output-only
    // coincidence cannot satisfy the semantic-preservation contract.
    for marker in [
        "SOVEREIGN STARS VOL.1 AWAKENING",
        "Celestial Witness",
        "第一章　星の声",
        "同行大見出し",
        "窓大見出し",
        "星間観測巫女",
        "星声受信器",
        "補足説明",
        "ゴマ傍点",
        "12",
        "ABC",
        "Google",
        "<rt>",
        "tcy",
        "upright",
        "text-emphasis",
        "text-decoration",
        "CSS-WARI-01",
        "L08-IMG-01",
        "L08-IMG-02",
    ] {
        assert!(
            source.contains(marker),
            "source fixture lacks semantic probe {marker:?}"
        );
        assert!(
            rawml.contains(marker),
            "KF8 RawML lost semantic probe {marker:?}"
        );
    }

    // Writing direction/root and metadata are represented in the record-zero
    // EXTH channel as well as the XHTML/CSS transport.
    assert_eq!(
        azw3.exth().text(503).as_deref(),
        Some("SOVEREIGN STARS VOL.1 AWAKENING")
    );
    assert_eq!(azw3.exth().text(525).as_deref(), Some("vertical-rl"));
    assert_eq!(azw3.exth().text(527).as_deref(), Some("rtl"));
    assert!(azw3.exth().text(100).as_deref() == Some("cbz-tools demo"));

    // Page/section and visible-TOC meaning remains represented by generated
    // documents, navigation labels, Guide and NCX/CTOC transport.
    let title = rawml
        .find("SOVEREIGN STARS VOL.1 AWAKENING")
        .expect("title-page marker is missing");
    let toc = rawml
        .find("目　次")
        .or_else(|| rawml.find("目 次"))
        .expect("visible TOC marker is missing");
    let body = rawml
        .find("Celestial Witness")
        .expect("bodymatter marker is missing");
    assert!(
        title < toc && toc < body,
        "title/TOC/body reading order changed"
    );
    assert!(
        rawml.contains("kindle:pos:fid:"),
        "internal position links were lost"
    );
    assert!(
        rawml.contains("kindle:embed:"),
        "resource references were lost"
    );
    assert!(
        rawml.contains("https://www.google.com"),
        "external Google hyperlink was lost"
    );
    assert!(
        azw3.image_records().len() >= 3,
        "cover/inline image resources were lost"
    );
    let mobi = azw3.mobi();
    azw3.assert_control_records();
    assert!(mobi.first_non_text > 2);
    assert!(
        azw3.assert_text_trailing_data() > 0,
        "PositionMap/TBS data was lost"
    );
    assert!(
        azw3.exth().u32(116).is_some(),
        "Start Reading/bodymatter route was lost"
    );
    assert!(azw3.record(mobi.ncx).starts_with(b"INDX"));
    assert!(azw3.record(mobi.guide).starts_with(b"INDX"));
    assert!(azw3.fdst_ranges().len() >= 2);

    // Exact glyph shape is intentionally not a renderer/device assertion;
    // UTF-8 source/output presence is the machine-verifiable portion of F-05.
    for marker in [
        "IVS試験：葛　辻　邊",
        "Latin：À Á Â Ä Å Æ Ç È É Ê Ë Ñ Ö Ø Ü ß",
        "補助漢字：𠮷　𠮟　𩸽",
        "結合濁点：が　ぎ　ぐ　げ　ご",
        "置換境界：－ ─ − ‐ ‑ 〜 ～ … ‥",
    ] {
        assert!(
            source.contains(marker),
            "source fixture lacks glyph probe {marker:?}"
        );
        assert!(rawml.contains(marker), "glyph probe {marker:?} was lost");
    }
}

#[test]
fn sovereign_stars_normalizes_cover_and_toc_landmarks_to_distinct_kf8_targets() {
    // A-11: a suppressed source cover document keeps its cover landmark tied
    // to the native cover resource, while the TOC remains a nav position.
    let source = epub_text(SOVEREIGN_EPUB);
    let source_landmarks = landmark_hrefs(&source);
    assert_eq!(
        source_landmarks
            .iter()
            .filter(|(kind, _)| kind.eq_ignore_ascii_case("cover"))
            .count(),
        1
    );
    assert_eq!(
        source_landmarks
            .iter()
            .filter(|(kind, _)| kind.eq_ignore_ascii_case("toc"))
            .count(),
        1
    );
    assert_eq!(
        source_landmarks
            .iter()
            .find(|(kind, _)| kind.eq_ignore_ascii_case("cover"))
            .map(|(_, href)| href.as_str()),
        Some("xhtml/cover.xhtml")
    );
    assert_eq!(
        source_landmarks
            .iter()
            .find(|(kind, _)| kind.eq_ignore_ascii_case("toc"))
            .map(|(_, href)| href.as_str()),
        Some("nav.xhtml")
    );
    let (source_cover_href, source_cover_mime) = source_cover_image_resource(&source);
    assert!(
        source_cover_mime.starts_with("image/"),
        "source cover-image metadata does not declare an image MIME"
    );
    assert!(
        source.contains(&format!(r#"xlink:href="../{source_cover_href}""#)),
        "source cover document does not reference its cover-image manifest resource"
    );

    let (_, azw3) = convert_fixture(SOVEREIGN_EPUB);
    let landmarks = reconstructed_landmark_hrefs(&azw3);
    assert_eq!(
        landmarks.len(),
        2,
        "generated nav must retain exactly one cover and one TOC landmark"
    );
    let cover_href = landmarks
        .iter()
        .find(|(kind, _)| kind.eq_ignore_ascii_case("cover"))
        .map(|(_, href)| href.as_str())
        .expect("generated cover landmark");
    let toc_href = landmarks
        .iter()
        .find(|(kind, _)| kind.eq_ignore_ascii_case("toc"))
        .map(|(_, href)| href.as_str())
        .expect("generated TOC landmark");
    assert!(cover_href.starts_with("kindle:embed:"));
    assert!(toc_href.starts_with("kindle:pos:fid:"));
    assert_ne!(cover_href, toc_href);
    assert!(!cover_href.contains("#"));
    let reconstructed = azw3.reconstructed_xhtml_sections();
    assert!(
        reconstructed
            .iter()
            .all(|section| { !String::from_utf8_lossy(section).contains("kindle:cover-landmark") }),
        "cover landmark normalization marker leaked into reconstructed XHTML"
    );

    let (embed_index, mime) = decode_embed_href(cover_href);
    assert!(embed_index > 0);
    let mobi = azw3.mobi();
    let cover_offset = azw3.exth().u32(201).expect("native cover offset") as usize;
    let cover_record = mobi.first_image + cover_offset;
    assert_eq!(
        mobi.first_image + embed_index - 1,
        cover_record,
        "cover landmark must resolve to the EXTH 201 native cover resource"
    );
    let serialized_cover = azw3.record(cover_record);
    let serialized_mime = image::guess_format(serialized_cover)
        .expect("native cover bytes have an image MIME")
        .to_mime_type();
    assert_eq!(mime, serialized_mime);
    assert!(
        !serialized_cover.is_empty(),
        "native cover resource has no serialized bytes"
    );

    let (toc_section, toc_offset) = azw3.resolve_position_href(toc_href);
    assert!(toc_section < reconstructed.len());
    assert!(toc_offset < reconstructed[toc_section].len());
    let nav_sections = reconstructed
        .iter()
        .enumerate()
        .filter(|(_, section)| {
            let section = String::from_utf8_lossy(section);
            section.contains(r#"epub:type="landmarks""#) && section.contains(r#"epub:type="toc""#)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(nav_sections, vec![toc_section]);
}

#[test]
fn kf8_fcis_uses_evidenced_canonical_shape_for_nine_flows() {
    // D-11: the evidenced KindleGen/calibre-compatible KF8 shape is fixed at
    // 52 bytes with field @12 equal to 2. Its exact semantic name is
    // undocumented; neither value nor record length is an FDST flow count.
    let (_, azw3) = convert_fixture(SOVEREIGN_EPUB);
    let mobi = azw3.mobi();
    assert_eq!(
        azw3.fdst_ranges().len(),
        9,
        "Sovereign Stars FDST flow count"
    );
    assert!(
        mobi.fcis < azw3.record_count(),
        "FCIS pointer is out of range"
    );
    let fcis = azw3.record(mobi.fcis);
    assert_eq!(&fcis[..4], b"FCIS");
    assert_eq!(azw3.record_with_magic(b"FCIS"), Some(mobi.fcis));
    assert_eq!(fcis.len(), 52);
    assert_eq!(u32_be(fcis, 12), 2);
    assert_eq!(
        u32_be(fcis, 20),
        u32::try_from(azw3.palm_doc().text_length).expect("PalmDOC text length fits FCIS")
    );
}

fn reconstructed_landmark_hrefs(azw3: &Azw3) -> Vec<(String, String)> {
    azw3.reconstructed_xhtml_sections()
        .into_iter()
        .flat_map(|section| {
            let section = String::from_utf8(section).expect("reconstructed XHTML is UTF-8");
            landmark_hrefs(&section)
        })
        .collect()
}

fn landmark_hrefs(markup: &str) -> Vec<(String, String)> {
    let mut landmarks = Vec::new();
    let mut reader = Reader::from_str(markup);
    reader.config_mut().trim_text(false);
    loop {
        match reader
            .read_event()
            .unwrap_or_else(|error| panic!("parse landmark XHTML: {error}"))
        {
            Event::Start(event) | Event::Empty(event)
                if local_name(event.name().as_ref()).eq_ignore_ascii_case("a") =>
            {
                let Some(kind) = attribute(&event, "type") else {
                    continue;
                };
                if !matches!(kind.to_ascii_lowercase().as_str(), "cover" | "toc") {
                    continue;
                }
                if let Some(href) = attribute(&event, "href") {
                    landmarks.push((kind, href));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    landmarks
}

fn source_cover_image_resource(markup: &str) -> (String, String) {
    let mut reader = Reader::from_str(markup);
    reader.config_mut().trim_text(false);
    let mut resources = Vec::new();
    loop {
        match reader
            .read_event()
            .unwrap_or_else(|error| panic!("parse source OPF: {error}"))
        {
            Event::Start(event) | Event::Empty(event)
                if local_name(event.name().as_ref()).eq_ignore_ascii_case("item") =>
            {
                let Some(properties) = attribute(&event, "properties") else {
                    continue;
                };
                if properties
                    .split_whitespace()
                    .any(|property| property.eq_ignore_ascii_case("cover-image"))
                    && let (Some(href), Some(media_type)) =
                        (attribute(&event, "href"), attribute(&event, "media-type"))
                {
                    resources.push((href, media_type));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    assert_eq!(resources.len(), 1, "source has one cover-image resource");
    resources.pop().expect("cover-image resource was parsed")
}

fn decode_embed_href(href: &str) -> (usize, &str) {
    let payload = href
        .strip_prefix("kindle:embed:")
        .expect("cover landmark is a kindle embed reference");
    let (encoded, mime) = payload
        .split_once("?mime=")
        .expect("embed reference has a MIME query");
    let index = encoded.bytes().fold(0usize, |value, byte| {
        let digit = match byte {
            b'0'..=b'9' => usize::from(byte - b'0'),
            b'A'..=b'V' => usize::from(byte - b'A') + 10,
            _ => panic!("invalid KF8 base32 embed address"),
        };
        value * 32 + digit
    });
    (index, mime)
}

fn u32_be(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("u32 bounds"))
}

#[test]
fn sovereign_stars_materializes_ordered_lists_spans_and_writing_topology() {
    // C-14: ordered-list ordinal materialization and span AIDs.
    // C-15: source/output span and AID preservation.
    // C-16: canonical html.hltr/body/div.main.vrtl topology and writing metadata.
    let source = epub_text(SOVEREIGN_EPUB);
    let (_, azw3) = convert_fixture(SOVEREIGN_EPUB);
    let rawml = String::from_utf8(azw3.rawml()).expect("Sovereign RawML is UTF-8");
    let source_stats = parse_markup(&source);
    let output_stats = parse_markup(&rawml);
    let reconstructed_stats = azw3
        .reconstructed_xhtml_sections()
        .into_iter()
        .map(|section| {
            let section = String::from_utf8(section).expect("reconstructed XHTML is UTF-8");
            parse_markup(&section)
        })
        .collect::<Vec<_>>();
    let reconstructed_html_hltr = reconstructed_stats
        .iter()
        .map(|stats| stats.html_hltr)
        .sum::<usize>();
    let reconstructed_body_main_vrtl = reconstructed_stats
        .iter()
        .map(|stats| stats.body_main_vrtl)
        .sum::<usize>();
    let reconstructed_synthetic_outer_main = reconstructed_stats
        .iter()
        .map(|stats| stats.synthetic_outer_main)
        .sum::<usize>();
    assert_eq!(source_stats.ordered_li_count, 2825);
    assert_eq!(output_stats.explicit_ordered_li_count, 2825);
    assert_eq!(
        output_stats.explicit_li_values.iter().copied().max(),
        Some(2802)
    );
    assert_eq!(source_stats.spans, 95);
    assert_eq!(output_stats.spans, 95);
    assert_eq!(output_stats.span_aids, 95);
    assert!(
        source_stats
            .span_classes
            .is_subset(&output_stats.span_classes)
    );
    let source_largest_nested = source_stats
        .lists
        .iter()
        .filter(|list| list.parent.is_some())
        .flat_map(|list| list.ordinals.iter().copied())
        .max()
        .expect("source has a nested ordered list");
    assert_eq!(source_largest_nested, 2802);
    // The logical RawML stream is serialized as SKEL followed by FRAG
    // payloads, so the concatenated text is not itself one well-formed DOM.
    // Parse the generated li payloads and tie the direct nested/outer cases to
    // their source markers instead of relying on accidental stream nesting.
    assert_eq!(li_value_for_text(&rawml, "D16-0001"), Some(1));
    assert_eq!(li_value_for_text(&rawml, "D16-0002"), Some(2));
    assert_eq!(
        li_value_for_text(&rawml, "十三　監査残件チェック"),
        Some(2802)
    );
    assert_eq!(li_value_for_text(&rawml, "キャラクター紹介"), Some(7));

    assert!(output_stats.aids.iter().all(|aid| {
        !aid.is_empty()
            && aid
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'A'..=b'V'))
    }));
    assert_eq!(output_stats.aids.len(), output_stats.unique_aids.len());

    assert!(source_stats.html_hltr > 0);
    assert!(reconstructed_html_hltr > 0);
    assert!(source_stats.body_main_vrtl > 0);
    assert!(reconstructed_body_main_vrtl > 0);
    assert_eq!(source_stats.synthetic_outer_main, 0);
    assert_eq!(reconstructed_synthetic_outer_main, 0);
    assert_eq!(source_stats.body_main_vrtl, reconstructed_body_main_vrtl);
    assert_eq!(azw3.exth().text(525).as_deref(), Some("vertical-rl"));
    assert_eq!(azw3.exth().text(527).as_deref(), Some("rtl"));
}

#[derive(Debug, Default)]
struct MarkupStats {
    lists: Vec<OrderedList>,
    ordered_li_count: usize,
    explicit_ordered_li_count: usize,
    spans: usize,
    span_aids: usize,
    span_classes: HashSet<String>,
    aids: Vec<String>,
    unique_aids: HashSet<String>,
    explicit_li_values: Vec<i64>,
    html_hltr: usize,
    body_main_vrtl: usize,
    synthetic_outer_main: usize,
}

#[derive(Debug)]
struct OrderedList {
    ordinals: Vec<i64>,
    next_ordinal: i64,
    parent: Option<(usize, usize)>,
}

#[derive(Debug)]
struct OpenElement {
    name: String,
    list: Option<usize>,
    main: bool,
    main_vrtl: bool,
}

fn parse_markup(markup: &str) -> MarkupStats {
    let mut reader = Reader::from_str(markup);
    reader.config_mut().trim_text(false);
    let mut stats = MarkupStats::default();
    let mut stack = Vec::<OpenElement>::new();

    loop {
        let event = reader
            .read_event()
            .unwrap_or_else(|error| panic!("parse generated/source markup: {error}"));
        match event {
            Event::Start(event) => {
                visit_start(&event, false, &mut stats, &stack);
                push_element(&event, &mut stats, &mut stack);
            }
            Event::Empty(event) => visit_start(&event, true, &mut stats, &stack),
            Event::End(event) => {
                let event_name = event.name();
                let name = local_name(event_name.as_ref());
                if let Some(index) = stack
                    .iter()
                    .rposition(|element| element.name.eq_ignore_ascii_case(name))
                {
                    stack.truncate(index);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    stats
}

fn visit_start(
    event: &BytesStart<'_>,
    _empty: bool,
    stats: &mut MarkupStats,
    stack: &[OpenElement],
) {
    let event_name = event.name();
    let name = local_name(event_name.as_ref());
    let class = attribute(event, "class");
    if name.eq_ignore_ascii_case("span") {
        stats.spans += 1;
        if attribute(event, "aid").is_some() {
            stats.span_aids += 1;
        }
        if let Some(class) = class.as_deref() {
            stats
                .span_classes
                .extend(class.split_whitespace().map(str::to_owned));
        }
    }
    if name.eq_ignore_ascii_case("html")
        && class
            .as_deref()
            .is_some_and(|value| has_class(value, "hltr"))
    {
        stats.html_hltr += 1;
    }
    if name.eq_ignore_ascii_case("div")
        && class
            .as_deref()
            .is_some_and(|value| has_class(value, "main"))
    {
        if class
            .as_deref()
            .is_some_and(|value| has_class(value, "vrtl"))
        {
            if stack
                .last()
                .is_some_and(|element| element.name.eq_ignore_ascii_case("body"))
            {
                stats.body_main_vrtl += 1;
            }
            if stack
                .last()
                .is_some_and(|element| element.main && !element.main_vrtl)
            {
                stats.synthetic_outer_main += 1;
            }
        } else {
            // Ordinary source `div.main` elements are valid and are not the
            // synthetic wrapper under audit. Only the direct main -> vrtl
            // nesting above is classified as the workaround.
        }
    }
    if let Some(aid) = attribute(event, "aid") {
        stats.aids.push(aid.clone());
        stats.unique_aids.insert(aid);
    }
    if name.eq_ignore_ascii_case("li") {
        let direct_ordered_parent = stack
            .last()
            .is_some_and(|element| element.name.eq_ignore_ascii_case("ol"));
        if direct_ordered_parent {
            stats.ordered_li_count += 1;
        }
        if let Some(value) = attribute(event, "value") {
            stats.explicit_ordered_li_count += 1;
            if let Ok(value) = value.trim().parse::<i64>() {
                stats.explicit_li_values.push(value);
            }
        }
    }
}

fn push_element(event: &BytesStart<'_>, stats: &mut MarkupStats, stack: &mut Vec<OpenElement>) {
    let event_name = event.name();
    let name = local_name(event_name.as_ref()).to_owned();
    let list = if name.eq_ignore_ascii_case("ol") {
        let parent = stack.iter().rev().find_map(|element| element.list);
        let parent = parent.map(|parent| (parent, stats.lists[parent].ordinals.len()));
        let next_ordinal = attribute(event, "start")
            .and_then(|value| value.trim().parse::<i64>().ok())
            .unwrap_or(1);
        let index = stats.lists.len();
        stats.lists.push(OrderedList {
            ordinals: Vec::new(),
            next_ordinal,
            parent,
        });
        Some(index)
    } else {
        None
    };
    if name.eq_ignore_ascii_case("li") && stack.last().and_then(|element| element.list).is_some() {
        let list = stack.last().and_then(|element| element.list).unwrap();
        let ordinal = attribute(event, "value")
            .and_then(|value| value.trim().parse::<i64>().ok())
            .unwrap_or(stats.lists[list].next_ordinal);
        stats.lists[list].ordinals.push(ordinal);
        stats.lists[list].next_ordinal = ordinal + 1;
    }
    let class = attribute(event, "class");
    let main = name.eq_ignore_ascii_case("div")
        && class
            .as_deref()
            .is_some_and(|value| has_class(value, "main"));
    let main_vrtl = main
        && class
            .as_deref()
            .is_some_and(|value| has_class(value, "vrtl"));
    stack.push(OpenElement {
        name,
        list,
        main,
        main_vrtl,
    });
}

#[derive(Debug)]
struct PendingListItem {
    value: Option<i64>,
    text: String,
}

fn li_value_for_text(markup: &str, marker: &str) -> Option<i64> {
    let mut reader = Reader::from_str(markup);
    reader.config_mut().trim_text(false);
    let mut items = Vec::<PendingListItem>::new();
    let mut completed = Vec::<(i64, String)>::new();

    loop {
        let event = reader
            .read_event()
            .unwrap_or_else(|error| panic!("parse generated list payload: {error}"));
        match event {
            Event::Start(event) if local_name(event.name().as_ref()).eq_ignore_ascii_case("li") => {
                items.push(PendingListItem {
                    value: attribute(&event, "value").and_then(|value| value.parse().ok()),
                    text: String::new(),
                });
            }
            Event::Text(event) => {
                let text = event
                    .unescape()
                    .unwrap_or_else(|error| panic!("decode generated list text: {error}"));
                for item in &mut items {
                    item.text.push_str(&text);
                }
            }
            Event::CData(event) => {
                let text = String::from_utf8_lossy(event.as_ref());
                for item in &mut items {
                    item.text.push_str(&text);
                }
            }
            Event::End(event) if local_name(event.name().as_ref()).eq_ignore_ascii_case("li") => {
                if let Some(index) = items.iter().rposition(|item| item.value.is_some()) {
                    let item = items.remove(index);
                    if let Some(value) = item.value {
                        completed.push((value, item.text));
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    completed
        .into_iter()
        .find_map(|(value, text)| text.contains(marker).then_some(value))
}

fn attribute(event: &BytesStart<'_>, wanted: &str) -> Option<String> {
    event
        .attributes()
        .filter_map(Result::ok)
        .find_map(|attribute| {
            (local_name(attribute.key.as_ref()).eq_ignore_ascii_case(wanted))
                .then(|| {
                    attribute
                        .unescape_value()
                        .ok()
                        .map(|value| value.into_owned())
                })
                .flatten()
        })
}

fn local_name(name: &[u8]) -> &str {
    let name = std::str::from_utf8(name).expect("markup names are UTF-8");
    name.rsplit(':').next().unwrap_or(name)
}

fn has_class(value: &str, wanted: &str) -> bool {
    value
        .split_whitespace()
        .any(|class| class.eq_ignore_ascii_case(wanted))
}
