# E2E and characterization architecture

The authoritative requirements and Audit IDs live in
[`docs/EPUB-to-KF8-Conversion-Audit.md`](../docs/EPUB-to-KF8-Conversion-Audit.md).
This directory keeps release contracts, diagnostic observations, fixtures, and
generic inspection infrastructure separate while preserving exact test-level
traceability.

## Suite structure

- `canonical_book_e2e.rs` is the independent canonical Sovereign Stars contract:
  semantic/KF8 output, cover and TOC normalization, FCIS, ordered lists, spans,
  and writing topology.
- `large_fixture_e2e.rs` is the independent large-input contract for Crime and
  Punishment, PalmDOC legacy equality, and RawML/INDX geometry.
- `api_cli_e2e.rs` is the independent public API, CLI process-boundary, and
  thread-safety contract.
- `characterization.rs` groups diagnostic EPUB and CSS observations under
  `characterization/`. These tests record current behavior and do not define
  release support unless the audit explicitly cites them as supporting evidence.
- `e2e.rs` is the release suite, organized by conversion responsibility:
  `ocf_container`, `package_metadata`, `resource_resolution`,
  `content_documents`, `navigation`, `css_safety`, `css_transport`,
  `layout_rendition_media`, `accessibility`, `cover_resources`,
  `embedded_fonts`, `fixed_layout`, `mixed_layout`, `resc`, and `unicode`.

## Release responsibility modules

| Module | Primary audit coverage | Responsibility |
| --- | --- | --- |
| `e2e.rs::ocf_container` | G1-03, G1-04, G1-08..G1-11, G2-12, G2-17..G2-20 | OCF rootfiles, mimetype, companion metadata, and manifest properties |
| `e2e.rs::package_metadata` | A-02, G2-07..G2-14, G5-10, G5-16 | Refinements, Dublin Core-related package projection, collections, fallback, and package-level semantics |
| `e2e.rs::resource_resolution` | G1-05, G2-06, G2-24, G8-01..G8-05 | OCF href resolution and explicit scripting/form safety boundaries |
| `e2e.rs::content_documents` | G3-02, G3-03, G3-08, G3-09, G3-13, G3-16..G3-20, G5-11, G5-13 | XHTML semantics, content resources, links, picture fallback, and content hierarchy |
| `e2e.rs::navigation` | A-04, A-06..A-08, A-12, D-06..D-08, F-06, F-10..F-12 | Nav, NCX, page-list, linear semantics, bodymatter, and cross-document navigation |
| `e2e.rs::css_safety` | G7-07, G7-09, G7-10, G7-25, G7-26 | Unsupported CSS detection, input surfaces, false-positive exclusions, generated content, and counters |
| `e2e.rs::css_transport` | G7-03, G7-06, G7-08, G7-12, G7-16, G7-27, G7-28; KCSS supporting evidence | URL rewriting and preservation, selectors, at-rules, custom properties, variables, and writer behavior |
| `e2e.rs::layout_rendition_media` | G2-25, G3-07, G6-14 | MathML safety and validated viewport/rendition boundaries |
| `e2e.rs::accessibility` | G6-22..G6-25, G8-06..G8-14 | Audio/video/media-overlay safety, generic resources, accessibility metadata, ARIA, and reading order |
| `e2e.rs::cover_resources` | A-08..A-11, B-15..B-17, E-04..E-10, F-12 | Cover resources, thumbnails, landmarks, cover suppression, and fixed-layout cover metadata |
| `e2e.rs::embedded_fonts` | E-12, G1-06, G1-07, G6-04..G6-11, G6-14, G6-19 | Embedded-font addressing, IDPF obfuscation, encryption validation, and related rendition resource paths |
| `e2e.rs::fixed_layout` | A-17b..A-17f, B-08..B-10, B-12..B-14, C-05..C-13, E-14 | Full fixed-layout page-flow and image-resource topology |
| `e2e.rs::mixed_layout` | A-15, A-16, C-05..C-12, E-14, F-09 | Item-level pre-paginated pages embedded between reflowable sections |
| `e2e.rs::resc` | D-17..D-22, G6-08, G6-09, G6-26 | RESC geometry, source spine order/idref, SKEL mapping, page-spread properties, and suppressed-cover consistency |
| `e2e.rs::unicode` | F-05 | IVS, combining marks, supplementary scalars, and exact RawML transport |

## Diagnostic characterization modules

- `characterization.rs::epub_input` records encoding, fallback, remote-resource,
  content-document, navigation, metadata, rendition, bidi, and CSS observations.
- `characterization.rs::epub_input_boundaries` records input-boundary matrices
  for font, MathML, language/bidi, rendition, SVG-resource, and image semantics.
- `characterization.rs::epub_css` records the broad CSS source/KF8 transport
  matrix; `epub_css_capabilities` records remaining capability observations.
- `characterization.rs::kindle_css` retains KindleGen comparison and control
  evidence. The four current writer-behavior contracts for style attributes,
  comment-prefixed declarations, function/semicolon parsing, and active
  stylesheet graphs are formal release E2E tests under
  `e2e.rs::css_transport::kindle_writer`.

Diagnostic tests preserve their observation points, fixtures, and safe-rejection
expectations. They are not substituted for release ownership.

## Fixture ownership

Checked-in fixtures are repository-owned inputs under
`fixtures/sovereign-stars/`, `fixtures/crime-and-punishment/source.epub`, and
`fixtures/embedded-font/source.epub`. Public API/CLI provenance is kept in
`fixtures/public-api-and-cli/source.txt`, while normal tests use the checked-in
EPUB directly. Navigation, cover, Unicode, and fixed-page topology use
independent in-memory recipes from `tests/support`.

Generic inspection infrastructure lives in `tests/support/`: conversion results,
ZIP EPUB construction, RawML/FDST/INDX/EXTH parsing, navigation recipes, Kindle
base32, SHA-1, IDPF obfuscation support, and fixture loading. Domain-specific
package, body, CSS-builder, and KindleGen-parser logic remains in its owning
module.

## Exact traceability

Primary E2E tests use stable `E2E-<DOMAIN>-NN` IDs. Part VI of the audit is
authoritative; function names are not stable audit IDs. Characterization and
supporting tests do not receive primary E2E IDs.

The following release functions provide direct reverse navigation for the A–F
contracts and canonical fixture obligations:

| E2E function | Primary Audit IDs |
| --- | --- |
| `e2e.rs::package_metadata::metadata_refinements_project_without_collection_leakage` | A-02, G2-07..G2-11 |
| `e2e.rs::cover_resources::cover_resource_and_library_thumbnail_contract_is_structurally_valid` | B-15..B-17, E-04..E-10, F-08, F-12 |
| `e2e.rs::navigation::navigation_sources_preserve_visible_toc_and_reading_order` | A-04, A-06..A-08, A-12, D-06..D-08, F-06, F-10..F-12 |
| `e2e.rs::unicode::unicode_scalars_survive_source_xhtml_and_kf8_rawml_without_normalization` | F-05 |
| `large_fixture_e2e.rs::legacy_compression::palmdoc_payloads_remain_byte_identical_to_the_legacy_encoder` | B-02, B-05a |
| `large_fixture_e2e.rs::index_geometry::crime_and_punishment_exercises_large_rawml_and_index_geometry` | D-01..D-05, D-15 |
| `large_fixture_e2e.rs::index_geometry::sovereign_stars_exercises_multi_detail_indx_geometry` | D-16 |
| `e2e.rs::mixed_layout::item_level_pre_paginated_semantic_lowers_to_a_dedicated_svg_flow` | A-15, A-16, C-05..C-12, E-14, F-09 |
| `e2e.rs::fixed_layout::fixed_layout_page_images_use_secondary_svg_flows` | A-17b..A-17f, B-08..B-10, B-12, C-05..C-11, C-13, E-14 |
| `e2e.rs::resc::resc_projects_spine_semantics_and_skel_topology` | D-17..D-22, G6-08, G6-09, G6-26 |
| `e2e.rs::embedded_fonts::checkin::embedded_font_is_preserved_as_a_resolvable_kf8_resource` | E-12 |
| `canonical_book_e2e.rs::sovereign_stars_normalizes_cover_and_toc_landmarks_to_distinct_kf8_targets` | A-11 |
| `canonical_book_e2e.rs::sovereign_stars_materializes_ordered_lists_spans_and_writing_topology` | C-14..C-16 |
| `canonical_book_e2e.rs::kf8_fcis_uses_evidenced_canonical_shape_for_nine_flows` | D-11 |

The audit’s G1–G8 and KCSS evidence cells are the authoritative reverse index
for the responsibility modules and their exact functions. Audit IDs remain stable
when source files or module paths change.
