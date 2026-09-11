# EPUB to KF8/AZW3 Conversion Audit

> Authoritative current-state audit for `AozoraEpub3-to-azw3`.
>
> This document records the converter's **current contracts, evidence, E2E traceability,
> known boundaries, and Physical Kindle observations**.

## 1. Purpose

This audit follows the supported conversion path end to end:

```text
valid EPUB input
→ EPUB semantic ingestion
→ Kindle/KF8 projection
→ KF8/AZW3 binary structure
→ writer compatibility
→ Physical Kindle acceptance where applicable
```

It does **not** require byte-identical output with KindleGen. Same-input KindleGen output
is writer-reference evidence where KF8/MOBI behavior is undocumented or reverse-engineered.
Physical Kindle is the final acceptance oracle for reader-visible failures, but is not
used as a complete binary-format, CSS, SVG, or renderer-capability specification.

## 2. Audit model

The ID namespaces are intentionally preserved:

- `G1`–`G8` — EPUB 3.3 input feature coverage and safe handling.
- `A`–`F` — KF8/AZW3 output contracts and semantic preservation.
- `KCSS` — Kindle CSS reader-support / writer-behavior / converter-action boundary.
- `L` — Physical Kindle device evidence.

An input feature and an output contract may intentionally have different IDs. Their
relationship is traceability, not duplication.

## 3. Evidence classes and policy

| Evidence | Meaning |
|---|---|
| `SPEC` | EPUB 3.3 semantic validity and reading-system semantics; does **not** establish KF8 serialization |
| `AMAZON-GUIDE` | Amazon publisher/reader-visible semantics, including fixed-layout and synthetic-spread/page-layout behavior; does **not** establish binary record placement |
| `SOURCE` | Current production-source inspection |
| `REFERENCE` | KF8/MOBI technical or implementation reference used to interpret observed structures; not normative Amazon feature semantics |
| `KINDLEGEN-FINAL` | Same-input KindleGen final-output serialization evidence across RawML, CSS, EXTH, RESC, SKEL, FRAG, INDX, and resources |
| `KINDLEGEN` | Legacy umbrella for same-input KindleGen writer evidence; closure claims must qualify it as `KINDLEGEN-FINAL` or `KINDLEGEN-LOG` |
| `KINDLEGEN-LOG` | KindleGen diagnostics/warnings; never normative by itself |
| `SELF-FINAL` | Reconstructed output from this converter |
| `E2E` | Executable integration/end-to-end evidence |
| `DEVICE` | Physical Kindle evidence |
| `PRODUCT-CONTRACT` | Explicit converter behavior guaranteed by this project |
| `PRODUCT-SCOPE` | Explicit supported / extended boundary |
| `CANONICAL-INPUT` | Behavior required by canonical AozoraEpub3 input |

Evidence priority is context-dependent, but the core rules are:

1. EPUB validity comes from EPUB/W3C specifications.
2. Kindle target behavior uses current Amazon guidance plus KF8/MOBI references.
3. Same-input KindleGen final output is stronger writer evidence than KindleGen warnings.
4. Amazon reader support is **not** the same thing as KindleGen writer behavior.
5. Unsupported input must not silently corrupt or lose meaningful semantics.
6. Do not invent vendor fields, payloads, or serialization rules when evidence is insufficient.

Closure rule: a claim that a feature has **no projection** or is **safe to ignore** may
be closed only after the applicable EPUB semantic check, Amazon reader-visible semantic
check, same-input KindleGen final-output check, and relevant self-output checks have all
been considered. The self-output checks include RawML/CSS and, where applicable, EXTH,
RESC, INDX, resource, SKEL, and FRAG structure. `REFERENCE` may interpret the observed
serialization, but cannot replace the EPUB or Amazon semantic checks. Current absence of
a field or structure in this self-writer is recorded as **current self-writer absence**;
it must not be promoted to a claim that KF8 has no such capability or serialization.

## 4. Current summary

```text
EPUB 3.3 primary coverage (G1–G8): 163 total
CLOSED:                               163
CHARACTERIZE:                           0
IMPLEMENTATION REQUIRED:                0
Known EPUB 3.3 inventory gaps:           0

Amazon supplemental input audit:
- KLAY-01..KLAY-03: CLOSED within observed writer-projection evidence boundary
- KLAY-04: CLOSED within observed writer-projection evidence boundary
- KLAY-05: CLOSED within observed writer-projection evidence boundary
- KAMZ-01: CLOSED within explicit book-type projection boundary
- KAMZ-02: CLOSED within XHTML markup-transport boundary; EXTH 132 conditions/metadata remain unestablished and are not an implementation requirement

Mandatory A–F canonical path:
- no known semantic mismatch
- machine-verifiable KF8/AZW3 geometry covered by the E2E suite

Conditional / diagnostic boundaries:
- B-11 / D-13: diagnostic; not release repair targets without stronger evidence
- C-04 / E-13: conditional / non-canonical; do not block the current canonical release path unless exercised

Kindle CSS compatibility:
- current KCSS matrix closed for demonstrated converter input surfaces and boundaries

Physical Kindle:
- L-01..L-13: 13 / 13 CONFIRMED PASS
- known Physical Kindle failures attributable to self conversion: 0
```

---

# Part I — EPUB 3.3 Input Coverage

## G1 — OCF Container

| ID | EPUB Feature | Coverage | Source Inspection | A–F Mapping | E2E Evidence | Safety | Closure |
|---|---|---|---|---|---|---|---|
| G1-01 | OCF ZIP container ingestion | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A source precondition | `E2E-OCF-01`; `E2E-OCF-03` | `PROJECTED` | `CLOSED` |
| G1-02 | `META-INF/container.xml` rootfile resolution | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A source precondition | `E2E-OCF-01`; `E2E-OCF-02` | `PROJECTED` | `CLOSED` |
| G1-03 | Multiple rootfiles / Multiple Renditions | `SUPPORTED` within default-rendition boundary | `src/epub/opf.rs::parse_rootfile` selects the first `rootfile` as the default package | — | `E2E-OCF-01` | `PROJECTED`; first-rootfile content is emitted without merging later renditions | `CLOSED` |
| G1-04 | Strict OCF `mimetype` validation | `INTENTIONALLY UNPROJECTED` | `src/epub/package.rs::parse_epub` does not use the OCF `mimetype` entry to derive package semantics | — | `E2E-OCF-02` | `SAFELY IGNORED`; valid, missing, wrong-value, late, and compressed shapes preserve the same body/title observable | `CLOSED` |
| G1-05 | OCF path / filename edge cases | `SUPPORTED` within valid URL/ZIP-name boundary | `src/xhtml/path.rs::percent_decode` and `src/epub/package.rs::resolve_href` decode valid percent-encoded path bytes before ZIP lookup while preserving relative, fragment/query, nested, Unicode, and case-sensitive semantics | A-03 related | `E2E-OCF-04` | `PROJECTED`; valid local references resolve to the addressed ZIP entries; external/data references remain outside ZIP lookup | `CLOSED` within the characterized valid OCF URL boundary |
| G1-06 | `META-INF/encryption.xml` | `SUPPORTED / SAFE REJECT` | `src/epub/package.rs` strictly resolves the OCF/XML Encryption namespaces and exact encryption hierarchy, validates IDPF algorithm/media type/ZIP target, and rejects malformed or real-encryption entries explicitly | E-12 related | `E2E-FONT-03`; supporting characterization | `PROJECTED` for IDPF font obfuscation; unknown algorithm, malformed metadata/namespace/structure, missing target, and non-font target are explicitly rejected | `CLOSED` |
| G1-07 | EPUB font obfuscation | `SUPPORTED` | `src/epub/package.rs` resolves the package `unique-identifier` target, removes XML whitespace, SHA-1s UTF-8, and XOR-deobfuscates the first 1040 bytes before the existing font path | E-12 related | `E2E-FONT-02` | `PROJECTED`; invalid unique identifiers and unsupported algorithms do not continue with obfuscated bytes | `CLOSED` |
| G1-08 | `META-INF/manifest.xml` | `INTENTIONALLY UNPROJECTED` | `src/epub/package.rs::parse_epub` consumes `container.xml`, the selected OPF, and manifest resources; the ancillary OCF manifest is not a publication-semantic input | — | `E2E-OCF-03` | `SAFELY IGNORED`; body, title, and output markers are unchanged | `CLOSED` |
| G1-09 | `META-INF/metadata.xml` | `INTENTIONALLY UNPROJECTED` | `src/epub/package.rs::parse_epub` uses selected package metadata and does not consume container `metadata.xml` | — | `E2E-OCF-03` | `SAFELY IGNORED`; a conflicting container title does not override the selected package title | `CLOSED` |
| G1-10 | `META-INF/rights.xml` | `OUT OF SCOPE` | `src/epub/package.rs::parse_epub` does not interpret opaque OCF rights metadata; this is separate from `encryption.xml` handling | — | `E2E-OCF-03` | `SAFELY IGNORED`; opaque rights input does not alter body/resource/metadata output | `CLOSED` |
| G1-11 | `META-INF/signatures.xml` | `OUT OF SCOPE` | `src/epub/package.rs::parse_epub` does not verify or project input EPUB signatures | — | `E2E-OCF-03` | `SAFELY IGNORED`; signature metadata is not treated as publication content | `CLOSED` |

---

## G2 — Package / Metadata / Resource Model

| ID | EPUB Feature | Coverage | Source Inspection | A–F Mapping | E2E Evidence | Safety | Closure |
|---|---|---|---|---|---|---|---|
| G2-01 | Manifest item identity | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-03 | `E2E-CANON-01` | `PROJECTED` | `CLOSED` |
| G2-02 | Spine order | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-04, F-06 | `E2E-NAV-01..E2E-NAV-02`, `E2E-CANON-01` | `PROJECTED` | `CLOSED` |
| G2-03 | Spine `linear` semantics | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-04 | `E2E-NAV-01`; `E2E-NAV-02` | `PROJECTED` | `CLOSED` |
| G2-04 | `page-progression-direction` | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-05, B-20 | `E2E-CANON-01`; `E2E-COVER-04` | `PROJECTED` | `CLOSED` |
| G2-05 | Core package metadata | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-01, F-13 | `E2E-CANON-01` | `PROJECTED` | `CLOSED` |
| G2-06 | Optional Dublin Core metadata | `PARTIAL / SAFE DEGRADE` | `src/epub/opf.rs` parses the existing publisher/description channels and now retains plain `dc:contributor`; `src/kindle/normalize.rs` and `src/kf8/builder.rs` reuse EXTH 101/103/108; optional values without a confirmed channel remain outside the KF8 projection | A-01/A-02 | `E2E-META-01` | `PROJECTED` for publisher/description/contributor; `SAFELY DEGRADED` by omission for coverage/date/format/relation/rights/source/subject/type without an established KF8 channel | `CLOSED` within the defined optional-DC projection boundary |
| G2-07 | Metadata `refines` | `SUPPORTED` | `CONFIRMED IMPLEMENTED`: `src/epub/opf.rs::parse_opf` retains `MetadataRecord { id, property, refines, scheme, value }` and `finalize_metadata` resolves supported refinements | A-02 | `E2E-META-02` | `PROJECTED` for recognized refinements; generic records remain in Book metadata until projection | `CLOSED` |
| G2-08 | Creator `role` refinement | `PARTIAL / SAFE DEGRADE` | `src/epub/opf.rs::finalize_metadata` links `role` to each creator; `src/kindle/normalize.rs` maps `aut`/`author` to Author and other roles to Contributor | A-02 | `E2E-META-02` | `SAFE DEGRADE`: finer roles become KF8 Contributor rather than being silently dropped | `CLOSED` |
| G2-09 | `file-as` refinement | `SUPPORTED` | `src/epub/opf.rs::finalize_metadata` resolves linked `file-as`; `src/kf8/builder.rs` emits EXTH 508/517/522 | A-02 | `E2E-META-02` | `PROJECTED` to Title/Creator/Publisher File As | `CLOSED` |
| G2-10 | `title-type` refinement | `SUPPORTED / SAFE DEGRADE` | `src/epub/opf.rs::finalize_metadata` selects the linked `title-type=main`; non-main title types are not used to overwrite the book title | A-02 | `E2E-META-02` | `SAFE DEGRADE`: main title is stable; non-main title types have no unsupported KF8 channel | `CLOSED` |
| G2-11 | Collection metadata | `INTENTIONALLY UNPROJECTED` | `src/epub/opf.rs::finalize_metadata` recognizes `belongs-to-collection`, `collection-type`, and `group-position` into `CollectionMetadata`; KF8 normalization intentionally omits the non-standard projection | A-02 | `E2E-META-02` proves collection text does not enter EXTH or body/reading output | `SAFELY IGNORED`: collection metadata is isolated from title, creator, body, reading order, and TOC | `CLOSED` |
| G2-12 | Package `<link>` metadata | `INTENTIONALLY UNPROJECTED` | `src/epub/opf.rs::parse_opf` consumes the defined package metadata fields but does not retrieve or merge linked metadata records | — | `E2E-OCF-08` | `SAFELY IGNORED`; local and remote links do not enter reading order, metadata, or resource output | `CLOSED` |
| G2-13 | Manifest fallback chain | `SUPPORTED` | `src/epub/opf.rs::ManifestItem` retains `fallback`; `src/epub/package.rs::resolve_content_source` resolves the chain before resource/content construction | A-03 related | `E2E-META-03`; `characterization.rs::epub_input::fallback_matrix_remains_diagnostic` | `PROJECTED`; missing target, cycle, and unsupported terminal fail explicitly during fallback resolution | `CLOSED` |
| G2-14 | Foreign resource fallback | `SUPPORTED` | `src/epub/package.rs::resolve_content_source` selects the supported fallback even when it is outside the spine; the original spine position receives the resolved content | A-03 related | `E2E-META-03`; `characterization.rs::epub_input::fallback_matrix_remains_diagnostic` | `PROJECTED`; no successful reading-order entry is left without a content document | `CLOSED` |
| G2-15 | Remote resources | `PARTIAL / SAFE OMISSION` | `src/epub/package.rs` excludes external manifest hrefs before ZIP lookup; external references remain external in XHTML/CSS | — | `characterization.rs::epub_input::remote_resource_boundary_remains_diagnostic` covers manifest-only and XHTML/CSS-referenced HTTP/HTTPS/scheme-relative resources | `SAFELY IGNORED` for unpackageable remote resources; no accidental ZIP-path failure | `CLOSED` within external-resource omission boundary |
| G2-16 | `remote-resources` property | `PARTIAL / SAFE OMISSION` | `src/epub/package.rs` treats `remote-resources` as an explicit non-ZIP resource boundary while retaining local resources with the property | — | `characterization.rs::epub_input::remote_resource_boundary_remains_diagnostic` verifies local-property success and remote-property omission | `SAFELY IGNORED` for remote resources; local resources remain packageable | `CLOSED` within external-resource omission boundary |
| G2-17 | Data URLs | `INTENTIONALLY UNPROJECTED` at package publication-resource boundary | `src/xhtml/path.rs::is_external_reference` keeps `data:` out of ZIP-entry resolution; CSS embedded data URLs remain covered by G7-16 | — | `E2E-OCF-06`; `E2E-CSS-03` | `SAFELY IGNORED` for spec-prohibited package `item`/metadata-link data URLs; embedded CSS data URLs are transported by G7-16 | `CLOSED` within defined package-resource boundary |
| G2-18 | Package collections | `INTENTIONALLY UNPROJECTED` | `src/epub/opf.rs::parse_opf` does not merge package collection content into publication metadata or spine semantics | — | `E2E-OCF-07` | `SAFELY IGNORED`; empty, linked, and metadata-bearing collections do not displace title/body/reading output | `CLOSED` |
| G2-19 | Legacy OPF Guide input | `INTENTIONALLY UNPROJECTED` for EPUB 3 navigation | `src/epub/opf.rs::parse_opf` does not consume legacy `guide`; EPUB 3 nav remains handled by the navigation path | D-07 related | `E2E-OCF-09` | `SAFELY IGNORED`; guide-bearing and guide-free EPUB3-nav outputs are identical | `CLOSED` |
| G2-20 | Generic manifest properties | `PARTIAL / SAFE IGNORE` | `src/epub/opf.rs::ManifestItem` retains property tokens while feature-specific consumers inspect known properties; unknown tokens do not create new semantics | multiple A rows | `E2E-OCF-05` | `SAFELY IGNORED` for unknown properties; known `nav` behavior remains effective when an unknown token is present | `CLOSED` within unknown-property boundary |
| G2-21 | `nav` property | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-06 | `E2E-NAV-01..E2E-NAV-02` | `PROJECTED` | `CLOSED` |
| G2-22 | `cover-image` property | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-09 | `E2E-COVER-01..E2E-COVER-04` | `PROJECTED` | `CLOSED` |
| G2-23 | `svg` property | `PARTIAL` | `src/epub/opf.rs::ManifestItem` retains generic properties; generic SVG lowering is driven by media type and resource href rather than a dedicated `svg` property path | E-14 related | `characterization.rs::epub_input_boundaries::generic_svg_resource_matrix_is_observable` | `SAFELY IGNORED` as a property when the SVG media type/resource path is packageable; object-reference loss is tracked under G6-19 | `CLOSED` within the `<img>` resource boundary |
| G2-24 | `scripted` property | `OUT OF SCOPE / SAFE REJECT` | `src/epub/xhtml.rs::reject_scripting` evaluates actual spine XHTML and direct-SVG script/form semantics before KF8 lowering or SVG wrapping rather than treating the marker alone as executable; marker-only/data-block cases remain harmless | — | `E2E-SCRIPT-01`; `E2E-SCRIPT-02`; `E2E-SCRIPT-03` | `SAFELY REJECTED` for inline/external scripts, forms, and scripted fallback content; marker-only/data-block cases are safely accepted | `CLOSED` within the explicit scripting/form rejection boundary |
| G2-25 | `mathml` property | `UNSUPPORTED` | `src/epub/package.rs` rejects a manifest `mathml` property before resource/fallback transport; no typed MathML IR or KF8 lowering is claimed. Same-input KindleGen can succeed while KF8 RawML silently loses MathML content, so successful KindleGen conversion is not a safe self-writer acceptance signal. | — | `E2E-FXL-07` | `SAFELY REJECTED` with identifiable MathML/unsupported error; fallback resources are not transported | `CLOSED` |

---

## G3 — EPUB Content Documents

| ID | EPUB Feature | Coverage | Source Inspection | A–F Mapping | E2E Evidence | Safety | Closure |
|---|---|---|---|---|---|---|---|
| G3-01 | XHTML — canonical AozoraEpub3 subset | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A/C/F broad | `E2E-CANON-01` | `PROJECTED` | `CLOSED` |
| G3-02 | Arbitrary EPUB 3 XHTML | `PARTIAL / TRANSPORT` | `src/epub/xhtml.rs` and KF8 RawML preserve the tested valid XHTML source outside the canonical Aozora subset; converter-owned rewrites remain explicit | — | `E2E-CONTENT-01` | `PROJECTED` within the characterized source-transport boundary | `CLOSED` within documented boundary |
| G3-03 | Generic XHTML structural elements | `SUPPORTED / TRANSPORT` | Structural elements are retained by the source-preserving XHTML/RawML path; no browser-grade renderer claim is made | F-07 | `E2E-CONTENT-01` | `PROJECTED` for section/article/header/footer/figure/table and tested descendants | `CLOSED` within characterized element boundary |
| G3-04 | Inline SVG in XHTML | `PARTIAL` | `CONFIRMED PARTIAL`: existing XHTML transport preserves inline SVG source | C-07/E-14 cover generated SVG flow, not general inline SVG | `characterization.rs::epub_input::svg_mathml_and_content_document_boundaries_are_observable` | `TRANSPORTED`; renderer semantics remain outside this P0 | `CLOSED` within transport scope |
| G3-05 | SVG Content Document directly in spine | `SUPPORTED` | `src/epub/package.rs::svg_content_document` decodes the SVG source, applies `src/epub/xhtml.rs::reject_scripting` to the direct spine source, and creates a deterministic XHTML wrapper before KF8 lowering | C-07/E-14 related | `E2E-META-05`; `E2E-SCRIPT-02`; legacy `characterization.rs::epub_input::svg_mathml_and_content_document_boundaries_are_observable` | `PROJECTED` for safe SVG; scripted/form-bearing direct SVG is `SAFELY REJECTED` before wrapping | `CLOSED` |
| G3-06 | Fixed-page SVG generated during KF8 lowering | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | C-06..C-13, E-14 | `E2E-FXL-01`, `E2E-FXL-02` | `PROJECTED` | `CLOSED` |
| G3-07 | MathML | `UNSUPPORTED` | `src/epub/xhtml.rs::reject_mathml` resolves the actual MathML namespace and rejects `<math>` before XHTML RawML transport. Same-input KindleGen can succeed while KF8 RawML silently loses MathML content, so the self-writer keeps an explicit safe-rejection boundary. | — | `E2E-FXL-06`; `E2E-FXL-07` | `CONFIRMED UNSUPPORTED` / `SAFELY REJECTED`; ordinary HTML and custom namespaces are not false positives | `CLOSED` |
| G3-08 | `epub:type` structural semantics | `PARTIAL / TRANSPORT` | Known converter-consumed tokens retain their existing navigation/cover handling; unknown and structural tokens remain in XHTML/RawML | A-08, A-10..A-12 | `E2E-CONTENT-01` | `PROJECTED` for consumed channels and transported for unconsumed tokens | `CLOSED` within documented boundary |
| G3-09 | RDFa | `SUPPORTED / TRANSPORT` | XHTML source transport preserves the tested RDFa attributes; the converter does not claim to evaluate RDF graphs | — | `E2E-CONTENT-01` | `PROJECTED` through preserved markup | `CLOSED` within semantic transport scope |
| G3-10 | `xml:lang` changes | `SUPPORTED / TRANSPORT` | `src/epub/xhtml.rs` preserves `xml:lang` in source XHTML and reconstructed KF8 RawML; no separate typed language IR is required for the current transport contract | — | `characterization.rs::epub_input_boundaries::language_bidi_and_direction_transport_is_observable` | `PROJECTED` through preserved KF8 markup; physical/assistive rendering behavior is outside this converter-level audit | `CLOSED` within semantic transport scope |
| G3-11 | HTML `lang` changes | `SUPPORTED / TRANSPORT` | `src/epub/xhtml.rs` preserves HTML `lang` attributes in source XHTML and reconstructed KF8 RawML; no separate typed language IR is required for the current transport contract | — | `characterization.rs::epub_input_boundaries::language_bidi_and_direction_transport_is_observable` | `PROJECTED` through preserved KF8 markup; physical/assistive rendering behavior is outside this converter-level audit | `CLOSED` within semantic transport scope |
| G3-12 | `dir` / bidi semantics | `SUPPORTED / TRANSPORT` | Element `dir` is preserved in reconstructed KF8 markup; `src/book/style.rs` also recognizes CSS `direction`; no destructive bidi normalization is applied | F-01 related | `characterization.rs::epub_input_boundaries::language_bidi_and_direction_transport_is_observable` | `PROJECTED`: `dir`/bidi markup is preserved and KF8 has a consumed bidi HTML/CSS surface; renderer parity remains outside this audit | `CLOSED` within semantic transport scope |
| G3-13 | ARIA semantics | `PARTIAL / TRANSPORT` | ARIA attributes remain in reconstructed KF8 XHTML; assistive-reader behavior is outside this converter audit | — | `E2E-CONTENT-01` | `PROJECTED` within source-retention scope | `CLOSED` within semantic transport scope |
| G3-14 | Internal hyperlinks | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-13, F-14 | `E2E-CONTENT-02`; `E2E-NAV-01` | `PROJECTED` | `CLOSED` |
| G3-15 | Fragment targets | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-13, F-14 | `E2E-CONTENT-02`; `E2E-NAV-01` | `PROJECTED` | `CLOSED` |
| G3-16 | Cross-document hyperlinks | `SUPPORTED` | `src/kf8/rawml.rs::rewrite_internal_links` resolves relative, percent-encoded, Unicode, and fragment targets through the shared path resolver; missing document/fragment targets fail explicitly | A-13/F-14 related | `E2E-CONTENT-02` | `PROJECTED` to KF8 position links; broken targets are safely rejected | `CLOSED` |
| G3-17 | External hyperlinks | `SUPPORTED / TRANSPORT` | `src/xhtml/path.rs::is_external_reference` keeps HTTP/HTTPS, scheme-relative, `mailto`, and `tel` links out of internal-link rewriting | — | `E2E-CONTENT-02` | `PROJECTED` through preserved external hrefs; physical link activation is outside scope | `CLOSED` within transport scope |
| G3-18 | Images in reflowable content | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-08, E-01..E-03 | `E2E-CANON-01`; `E2E-COVER-01` | `PROJECTED` | `CLOSED` |
| G3-19 | `<picture>` / alternate image sources | `PARTIAL / SAFE REJECT` | `src/epub/xhtml.rs::reject_unsupported_srcset` rejects package-local `srcset` candidates before RawML; fallback-only `<picture><img>` uses the existing image-resource projection; external/data candidates remain transportable | — | `E2E-CONTENT-03` | `PROJECTED` for fallback-only pictures and external/data candidates; `SAFELY REJECTED` for unprojectable package-local `srcset` | `CLOSED` within documented boundary |
| G3-20 | HTML tables | `SUPPORTED / TRANSPORT` | Table markup and tested structural/association attributes remain in reconstructed XHTML/RawML; no renderer layout claim is made | F-07 related | `E2E-CONTENT-01` | `PROJECTED` through preserved markup | `CLOSED` within semantic transport scope |
| G3-21 | Ordered/unordered lists | `SUPPORTED` within tested subset | `CONFIRMED IMPLEMENTED` | C-14/F-07 | `E2E-CANON-01` | `PROJECTED` | `CLOSED` |
| G3-22 | Nested ordered lists | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | C-14 | `E2E-CANON-01` | `PROJECTED` | `CLOSED` |
| G3-23 | Ruby | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-02 | `E2E-CANON-01` | `PROJECTED` | `CLOSED` |
| G3-24 | TCY / text-combine | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-02 | `E2E-CANON-01` | `PROJECTED` | `CLOSED` |
| G3-25 | Emphasis marks | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-03 | `E2E-CANON-01` | `PROJECTED` | `CLOSED` |
| G3-26 | Canonical warichu | `SUPPORTED` within canonical subset | `CONFIRMED IMPLEMENTED` | F-04 | `E2E-CANON-01` | `PROJECTED` | `CLOSED` |
| G3-27 | Forms | `OUT OF SCOPE / SAFE REJECT` | `src/epub/xhtml.rs::reject_scripting` rejects HTML `form` elements in spine content before KF8 lowering | — | `E2E-SCRIPT-01` | `SAFELY REJECTED` with a feature-specific error; no successful AZW3 is produced | `CLOSED` within the explicit form rejection boundary |

---

## G4 — Character Encoding

| ID | EPUB Feature | Coverage | Source Inspection | A–F Mapping | E2E Evidence | Safety | Closure |
|---|---|---|---|---|---|---|---|
| G4-01 | UTF-8 XML/XHTML | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | broad A/F | `E2E-UNICODE-01`; `E2E-CANON-01` | `PROJECTED` | `CLOSED` |
| G4-02 | UTF-8 CSS | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-14/C-03 | `E2E-CSS-03`; `E2E-CANON-01` | `PROJECTED` | `CLOSED` |
| G4-03 | UTF-16 XML/XHTML | `SUPPORTED` | `src/epub/package.rs::decode_text_entry` detects BOM/no-BOM UTF-16LE/BE and strictly decodes scalar values before XHTML parsing | B-04/F-05 boundary | `characterization.rs::epub_input::encoding_matrix_remains_diagnostic` covers BOM+declaration, BOM-only, and no-BOM+declaration for both endian forms | `PROJECTED`; no replacement-character or missing-body path for valid inputs | `CLOSED` |
| G4-04 | UTF-16 CSS | `SUPPORTED` | `src/epub/package.rs::decode_text_entry` canonicalizes UTF-16 CSS to UTF-8; `src/kindle/normalize.rs` and `src/kf8/css_flow.rs` consume the strict canonical form | A-14/C-03 boundary | `characterization.rs::epub_input::encoding_matrix_remains_diagnostic` | `PROJECTED`; UTF-16LE/BE CSS markers survive with no NUL/replacement corruption | `CLOSED` |
| G4-05 | XML declaration encoding handling | `SUPPORTED` | `src/epub/package.rs::validate_declared_encoding` validates XML declaration against the selected decoder after BOM/byte-shape detection | B-04/F-05 boundary | `characterization.rs::epub_input::encoding_matrix_remains_diagnostic` includes a mismatched UTF-8/UTF-16 declaration and expects an explicit error | `SAFELY REJECTED` for declaration/byte mismatch; valid declarations are projected | `CLOSED` |
| G4-06 | CSS encoding declaration / `@charset` | `SUPPORTED` | `src/epub/package.rs::validate_declared_encoding` validates CSS `@charset` after strict decoding and rejects unsupported/mismatched declarations | A-14/C-03 boundary | `characterization.rs::epub_input::encoding_matrix_remains_diagnostic` covers UTF-8 and UTF-16 `@charset` forms | `PROJECTED`; invalid declaration is rejected without lossy continuation | `CLOSED` |
| G4-07 | Unicode scalar preservation | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-05 | `E2E-UNICODE-01` | `PROJECTED` | `CLOSED` |
| G4-08 | IVS | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-05 | `E2E-UNICODE-01` | `PROJECTED` | `CLOSED` |
| G4-09 | Combining marks | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-05 | `E2E-UNICODE-01` | `PROJECTED` | `CLOSED` |
| G4-10 | Supplementary-plane characters | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-05 | `E2E-UNICODE-01` | `PROJECTED` | `CLOSED` |
| G4-11 | No unintended Unicode normalization | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-05 | `E2E-UNICODE-01` | `PROJECTED` | `CLOSED` |

---

## G5 — Navigation

| ID | EPUB Feature | Coverage | Source Inspection | A–F Mapping | E2E Evidence | Safety | Closure |
|---|---|---|---|---|---|---|---|
| G5-01 | EPUB 3 TOC | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-06, A-07, F-10, F-11 | `E2E-NAV-01..E2E-NAV-02` | `PROJECTED` | `CLOSED` |
| G5-02 | Nested TOC hierarchy | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-07, F-10/F-11 | `E2E-NAV-01..E2E-NAV-02` | `PROJECTED` | `CLOSED` |
| G5-03 | Navigation labels | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-07/F-11 | `E2E-NAV-01..E2E-NAV-02` | `PROJECTED` | `CLOSED` |
| G5-04 | Navigation destinations | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-07/F-11 | `E2E-NAV-01..E2E-NAV-02` | `PROJECTED` | `CLOSED` |
| G5-05 | `landmarks` nav | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-08 | `E2E-NAV-01..E2E-NAV-02`, `E2E-COVER-01..E2E-COVER-04` | `PROJECTED` | `CLOSED` |
| G5-06 | Cover landmark | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-10/A-11 | `E2E-COVER-01..E2E-COVER-04` | `PROJECTED` | `CLOSED` |
| G5-07 | Bodymatter / text landmark | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-12, F-12 | `E2E-NAV-01..E2E-NAV-02` | `PROJECTED` | `CLOSED` |
| G5-08 | Title-page landmark | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-12 | `E2E-NAV-01..E2E-NAV-02` | `PROJECTED` | `CLOSED` |
| G5-09 | `page-list` nav | `SUPPORTED` | `src/book/mod.rs::Navigation` has a typed `page_list` channel; `src/epub/navigation.rs::append_nav_item` keeps it separate from TOC | D-06/F-10/F-11 boundary | `E2E-META-04` | `PROJECTED` to a dedicated generated KF8 page-list section | `CLOSED` |
| G5-10 | Page-break ↔ page-list integration | `SUPPORTED` | `src/epub/navigation.rs::canonicalize_navigation` resolves page-list targets; Kindle normalization preserves target fragments for position rewriting | D-06/F-10/F-11 boundary | `E2E-META-04` proves page label, target fragment, and `kindle:pos:fid:` linkage | `PROJECTED` | `CLOSED` |
| G5-11 | Other custom nav types | `PARTIAL / SAFE IGNORE` | `src/book/mod.rs::Navigation.custom` and `src/epub/navigation.rs::append_nav_item` retain custom groups in IR; KF8 normalization intentionally consumes only recognized TOC/page-list/landmark channels | — | `E2E-CONTENT-04` | `SAFELY IGNORED` for custom UI channels; visible TOC and landmarks remain separate | `CLOSED` |
| G5-12 | Multiple nav elements | `SUPPORTED` | `src/book/mod.rs::Navigation` separates `items`, `page_list`, `landmarks`, and typed `custom` groups; `src/epub/navigation.rs::append_nav_item` classifies each nav | A-06/A-08 | `E2E-META-04` | `PROJECTED` for recognized channels; custom nav is intentionally omitted without collapsing recognized channels | `CLOSED` |
| G5-13 | Unlinked `<span>` nav headings | `PARTIAL / SAFE DEGRADE` | `src/epub/navigation.rs::parse_nav_xhtml` retains span labels and child items without inventing hrefs; KF8 NCX omits the untargetable heading while linked descendants remain ordered, and the nav document preserves the source hierarchy | A-07 related | `E2E-CONTENT-04` | `SAFELY DEGRADED`: no fake link, child entries retained, source hierarchy remains in KF8 XHTML | `CLOSED` within documented KF8 boundary |
| G5-14 | Navigation image labels / `alt` | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-07/F-11 | parser + navigation coverage | `PROJECTED` | `CLOSED` |
| G5-15 | EPUB 2 NCX | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-06/D-06/F-11 | `E2E-NAV-01..E2E-NAV-02` | `PROJECTED` | `CLOSED` |
| G5-16 | EPUB 3 nav + NCX coexistence | `SUPPORTED` | `src/epub/package.rs` uses NCX for the logical TOC when present, then merges EPUB nav page-list/landmarks/custom channels independently | A-06/A-08 | `E2E-META-04`; `E2E-NAV-01..E2E-NAV-02` covers canonical NCX | `PROJECTED` with deterministic NCX logical-TOC precedence and independent EPUB channels | `CLOSED` |
| G5-17 | NCX hierarchy | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-07/F-11 | navigation E2E | `PROJECTED` | `CLOSED` |
| G5-18 | NCX labels / targets | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-07/F-11 | navigation E2E | `PROJECTED` | `CLOSED` |

---

## G6 — Layout / Publication Resources

EPUB 3.3 spread semantics are kept separate from Amazon page-layout semantics. The
`rendition:spread` value family is `auto`, `both`, `landscape`, `none`, and the
deprecated-but-still-defined `portrait`; the corresponding spine overrides include
`rendition:spread-portrait` as a deprecated property. The `rendition:page-spread-left`,
`rendition:page-spread-right`, and `rendition:page-spread-center` properties have
unprefixed aliases `page-spread-left` and `page-spread-right` in the EPUB input
vocabulary covered here. Left, right, and center page-spread values apply to both
pre-paginated and reflowable content. Center is the `spread-none` centering alias; it
does not by itself establish an Amazon binary record or placement contract. The
unprefixed Amazon `page-spread-center` source term is tracked separately in KLAY.

Closure boundary for the demonstrated contracts: invalid enum values still reject.
Publication `rendition:align-x-center` remains invalid because EPUB 3.3 defines the
property only as an itemref override. The RESC label and placement remain observed
KindleGen/self-writer behavior, not an Amazon-defined record specification.

| ID | EPUB Feature | Coverage | Source Inspection | A–F Mapping | E2E Evidence | Safety | Closure |
|---|---|---|---|---|---|---|---|
| G6-01 | Reflowable publication | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | C/F broad | `E2E-CANON-01`; `E2E-BINARY-02` | `PROJECTED` | `CLOSED` |
| G6-02 | Publication `rendition:layout` | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-15 | `E2E-FXL-02` | `PROJECTED` | `CLOSED` |
| G6-03 | Item-level `rendition:layout` | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-15/A-16/C-05..C-12/F-09 | `E2E-FXL-01` | `PROJECTED` | `CLOSED` |
| G6-04 | `rendition:orientation` | `SUPPORTED` | `src/epub/opf.rs` parses publication orientation into typed rendition IR; the KF8 writer preserves publication orientation in RESC metadata and emits EXTH 124 for fixed, mixed-layout, and reflowable publications through the existing orientation channel | A-17a | `E2E-RESC-01`; `E2E-RESOURCE-01`; `characterization.rs::epub_input_boundaries::rendition_spread_orientation_and_viewport_are_observable` | `PROJECTED` for portrait/landscape; `auto` maps to Kindle `none`; item-level orientation remains an independent RESC itemref property | `CLOSED` |
| G6-05 | Item-level orientation override | `SUPPORTED` | `rendition:orientation-auto/portrait/landscape` is parsed into typed rendition IR, retained on the Kindle section, and serialized from the original itemref property list; publication orientation remains the separate EXTH 124 channel. Same-input KindleGen final output retains explicit `rendition:orientation-auto` on the RESC `itemref`. | A-17a boundary | `E2E-RESC-02` asserts `rendition:orientation-auto` and publication EXTH 124 | `PROJECTED`; writer behavior characterized for all EPUB 3.3 values | `CLOSED` |
| G6-06 | Publication `rendition:spread` | `SUPPORTED` | Typed publication spread and the RESC writer represent `auto`, `both`, `landscape`, `none`, and deprecated `portrait`; same-input KindleGen final output retains explicit `rendition:spread=portrait` in RESC metadata. | D-17/D-21 related | `E2E-RESC-02` asserts all publication spread values including `portrait` | `PROJECTED` | `CLOSED` |
| G6-07 | Item-level spread override | `SUPPORTED` | Typed item spread and the original itemref property vector are retained through the Kindle IR; deprecated `rendition:spread-portrait` is serialized with source spelling/order. | D-18/D-19/D-21 | `E2E-RESC-02` asserts `rendition:spread-portrait` | `PROJECTED` | `CLOSED` |
| G6-08 | EPUB `rendition:page-spread-left` | `SUPPORTED` | Typed source property and `PageSpread::Left` are retained through the Kindle IR and serialized as a RESC spine itemref property; the source spelling is preserved. | D-17..D-21 | `E2E-RESC-03` | `PROJECTED`; source page-spread semantics are retained in final RESC | `CLOSED` |
| G6-09 | EPUB `rendition:page-spread-right` | `SUPPORTED` | Typed source property and `PageSpread::Right` are retained through the Kindle IR and serialized as a RESC spine itemref property; the source spelling is preserved. | D-17..D-21 | `E2E-RESC-03` | `PROJECTED`; source page-spread semantics are retained in final RESC | `CLOSED` |
| G6-10 | Publication `rendition:flow` | `SUPPORTED` | Typed publication flow includes `auto`, `paginated`, `scrolled-continuous`, and `scrolled-doc`; OPF parsing and RESC serialization preserve the value. Same-input KindleGen final output retains explicit `rendition:flow=auto` in RESC metadata. | D-17/D-21 related | `E2E-RESC-02` asserts all four publication flow values | `PROJECTED` | `CLOSED` |
| G6-11 | `rendition:align-x-center` | `SUPPORTED` | Typed item alignment remains distinct from page-spread semantics; the original `rendition:align-x-center` itemref property is serialized unchanged and is never converted to `page-spread-center`. | D-21 related | `E2E-RESC-02` | `PROJECTED` | `CLOSED` |
| G6-12 | Full fixed-layout publication | `SUPPORTED / EXTENDED` | `CONFIRMED IMPLEMENTED` | C-13 etc. | `E2E-FXL-02` | `PROJECTED` | `CLOSED` within extended scope |
| G6-13 | Mixed reflowable / pre-paginated | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | C-12/F-09 | `E2E-FXL-01` | `PROJECTED` | `CLOSED` |
| G6-14 | Viewport semantics | `SUPPORTED / SAFE REJECT` | `src/epub/xhtml.rs::validate_viewport` validates each fixed/pre-paginated XHTML document's local viewport grammar for positive numeric or device dimensions with comma, semicolon, and ASCII-whitespace separators; duplicate viewport metas remain document-local transport. `src/epub/package.rs` does not compare viewport values across documents or derive `Metadata.original_resolution`; explicit publication `original-resolution` remains the sole EXTH 126 source. | FXL related | `E2E-FXL-03`; `E2E-FXL-04`; `E2E-FXL-05` | `PROJECTED` as preserved XHTML viewport markup; varying mixed/fixed-page viewports, duplicate viewport metas, device dimensions, and differing explicit resolution convert successfully without deriving EXTH 126; partial, malformed, and duplicate dimension assignments are `SAFELY REJECTED`; SVG `viewBox` is not claimed | `CLOSED` |
| G6-15 | Original-resolution metadata | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-17c/B-12 | `E2E-FXL-02` | `PROJECTED` | `CLOSED` |
| G6-16 | Orientation-lock | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-17d/B-10 | `E2E-FXL-02` | `PROJECTED` | `CLOSED` |
| G6-17 | Primary writing mode | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-17f/B-19 | `E2E-CANON-03`; `E2E-FXL-02` | `PROJECTED` | `CLOSED` |
| G6-18 | Standard raster images | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | E-01..E-11/F-08 | `E2E-CANON-01`; `E2E-COVER-01` | `PROJECTED` | `CLOSED` |
| G6-19 | Generic SVG image resources | `SUPPORTED` | `src/kf8/rawml.rs::rewrite_section_assets` rewrites `src`/`xlink:href` and tag-gated `<object data>` through the existing resolver; generic HTML `data` attributes remain byte-for-byte untouched | E-14 related | `E2E-RESOURCE-01`; supporting characterization | `PROJECTED` for supported SVG object references; generic `<div data>` is preserved | `CLOSED` |
| G6-20 | Embedded plain fonts | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | E-12 | `E2E-FONT-01` | `PROJECTED` | `CLOSED` |
| G6-21 | Obfuscated embedded fonts | `SUPPORTED` | `src/epub/package.rs` deobfuscates IDPF targets before `KindleResource`/CSS reference generation | E-12 related | `E2E-FONT-02` | `PROJECTED` as a usable plain font resource | `CLOSED` |
| G6-22 | Generic non-image resources | `PARTIAL / TRANSPORT + SAFE REJECT` | `src/epub/package.rs` serializes unreferenced generic binary resources; `src/epub/xhtml.rs::reject_unsupported_generic_object` rejects package-local generic `<object data>` targets that have no faithful KF8 reference projection | E-13 boundary | `E2E-ACCESS-03` | `PROJECTED` for unreferenced resource-byte transport; referenced generic object is `SAFELY REJECTED` | `CLOSED` within the documented transport/rejection boundary |
| G6-23 | Audio resources | `OUT OF SCOPE / SAFE REJECT` | `src/epub/xhtml.rs::reject_media_playback` rejects audio elements in spine content before KF8 lowering; manifest-only audio remains non-reading ancillary data | — | `E2E-ACCESS-02`; `E2E-ACCESS-05` | Playback is `SAFELY REJECTED`; unlinked media is safely omitted from reading semantics | `CLOSED` |
| G6-24 | Video resources | `OUT OF SCOPE / SAFE REJECT` | `src/epub/xhtml.rs::reject_media_playback` rejects video elements in spine content before KF8 lowering; manifest-only video remains non-reading ancillary data | — | `E2E-ACCESS-02`; `E2E-ACCESS-05` | Playback is `SAFELY REJECTED`; unlinked media is safely omitted from reading semantics | `CLOSED` |
| G6-25 | Remote media resources | `OUT OF SCOPE / SAFE REJECT` | External media manifest entries are skipped by the shared external-reference boundary; remote `<audio>`/`<video>` playback is rejected by the same feature-specific detector | G2/G3 external-reference boundary | `E2E-ACCESS-02`; `E2E-ACCESS-05` | Remote playback is `SAFELY REJECTED`; unreferenced remote media is safely omitted | `CLOSED` |
| G6-26 | EPUB `rendition:page-spread-center` | `SUPPORTED` | Typed center input retention and the `spread-none` centering relationship remain separate from serialization; the center source property is preserved in the RESC spine itemref using KindleGen-final output as the writer reference. | D-17..D-21 | `E2E-RESC-03` | `PROJECTED`; source page-spread semantics are retained in final RESC | `CLOSED` |
| G6-27 | Item-level `rendition:flow-*` overrides | `SUPPORTED` | EPUB 3.3 defines `rendition:flow-auto`, `rendition:flow-paginated`, `rendition:flow-scrolled-continuous`, and `rendition:flow-scrolled-doc` on spine itemrefs; typed parsing accepts all four while the original property vector preserves source spelling/order. | D-18/D-19/D-21 related | `E2E-RESC-02` asserts all four item properties | `PROJECTED` | `CLOSED` |
| G6-28 | Deprecated `rendition:viewport` package property | `SUPPORTED` | EPUB 3.3 deprecated package `rendition:viewport` is ingested into an explicit metadata field and projected as publication RESC metadata; this remains distinct from the XHTML viewport/EXTH 126 contract in G6-14. | FXL boundary | `E2E-RESC-02` asserts RESC metadata position and value | `PROJECTED` | `CLOSED` |

---


## KLAY — Amazon-only page-layout/source semantics

`KLAY` is an independent Amazon evidence namespace. It does not mix Amazon extension
terms into the EPUB 3.3 `G1`–`G8` coverage table and is excluded from its row counts.
Amazon guidance establishes reader-visible/page-layout semantics; it does not specify a
KF8 binary record name or placement. KindleGen-final observations are therefore listed
separately from the Amazon semantic evidence.

| ID | Amazon page-layout/source semantic | Applicability and interaction | AMAZON-GUIDE evidence boundary | KindleGen/self boundary | Status |
|---|---|---|---|---|---|
| KLAY-01 | `page-spread-left` | In a landscape synthetic spread, requests the left viewport; an unpaired left page is treated as center. Source-side applicability is distinct from EPUB `rendition:page-spread-left` parsing. | Amazon reader-visible synthetic-spread/page-layout semantic only; no binary placement claim | Same-input final output and self RESC evidence retain the source page-spread property; binary record naming remains observational | `CLOSED` within observed writer-projection boundary |
| KLAY-02 | `page-spread-right` | In a landscape synthetic spread, requests the right viewport; an unpaired right page is treated as center. Source-side applicability is distinct from EPUB `rendition:page-spread-right` parsing. | Amazon reader-visible synthetic-spread/page-layout semantic only; no binary placement claim | Same-input final output and self RESC evidence retain the source page-spread property; binary record naming remains observational | `CLOSED` within observed writer-projection boundary |
| KLAY-03 | `page-spread-center` | In landscape, requests a single centered viewport; it is the Amazon page-layout counterpart to the EPUB center alias, not proof of a shared binary encoding. | Amazon reader-visible centering semantic only; no binary placement claim | Same-input final output records retention and self RESC coverage preserves the corresponding source property; no Amazon binary-format claim | `CLOSED` within observed writer-projection boundary |
| KLAY-04 | `facing-page-left` / `facing-page-right` and `layout-blank` | Facing-page direction supplies placement semantics. `layout-blank` supplies a synthetic-spread completion blank that is shown in landscape and ignored in portrait. | Amazon official page-layout/synthetic-spread semantic only | Same-input KindleGen final output and `E2E-RESC-02` retain `facing-page-left`, `facing-page-right`, and `layout-blank` on RESC `itemref` properties. Existing source-property transport is sufficient; no redundant production logic is added. | `CLOSED` within observed writer-projection boundary |
| KLAY-05 | `primary-writing-mode` ordering interaction | Writing-mode direction affects reader interpretation of left/right pages and `layout-blank` within a synthetic spread, while writer serialization preserves source spine order and source page-layout properties. | Amazon reader-visible ordering interaction only; it does not define RESC, SKEL, or other binary placement | Same-input KindleGen final comparison confirms that `horizontal-lr` vs `horizontal-rl` does not reorder RESC itemrefs, swap `facing-page-left`/`facing-page-right`, move `layout-blank`, or alter skelid order. EXTH 525 preserves `primary-writing-mode`; explicit `page-progression-direction` is preserved independently through EXTH 527 and the RESC spine attribute. Current self output already preserves the same independent channels, so no reorder/swap logic is required. | `CLOSED` within observed writer-projection boundary |


## KAMZ — Other Amazon-specific EPUB/XHTML input semantics

`KAMZ` is supplemental to the EPUB 3.3 G1–G8 count. A row belongs here only when an
Amazon-specific semantic can arrive through the converter's EPUB package/XHTML/CSS
input. KDP workflow, sales metadata, device-only behavior, authoring-size limits, and
preview/publishing-process requirements are not added merely because Amazon documents
them. Physical rendering remains a separate device concern.

| ID | Amazon-specific EPUB/XHTML input semantic | Input applicability | Converter/audit boundary | Status |
|---|---|---|---|---|
| KAMZ-01 | `book-type=children` | Amazon fixed-layout EPUB metadata may use `children`; this value is reachable from EPUB input independently of the canonical `comic` contract. | Same-input KindleGen final output emits EXTH 123=`children`; self KF8 E2E verifies explicit `children` emission and absence for unknown values. No book type is synthesized. | `CLOSED` within the explicit book-type projection boundary |
| KAMZ-02 | Region Magnification / `app-amzn-magnify` + `data-app-amzn-magnify` | Amazon fixed-layout XHTML can carry active-area/target markup from which Kindle tooling derives Region Magnification behavior/metadata. | Same-input KindleGen final output and self E2E preserve the magnification XHTML markup. EXTH 132 was not produced by the focused input; its conditions/metadata and trigger are not established. No EXTH 132 or `app-amzn-magnify` trigger is generated by this converter, and none is an implementation requirement from this evidence. | `CLOSED` within markup-transport boundary; EXTH 132 optional/trigger not established |

Amazon's fixed-layout authoring rule such as one HTML file per represented Kindle page
is retained as publishing guidance, not promoted to an independent converter semantic
row unless investigation shows that violating or transforming that structure changes a
reachable EPUB→KF8 projection contract. Likewise, Virtual Panels activation/rendering
is a reader/device behavior and is not a writer-closure requirement by itself.

## G7 — CSS / Presentation

CSS source preservation and semantic CSS interpretation are separate concerns.

A simple semantic parser does not automatically imply that every unsupported CSS construct is lost in output.

| ID | EPUB Feature | Coverage | Source Inspection | A–F Mapping | E2E Evidence | Safety | Closure |
|---|---|---|---|---|---|---|---|
| G7-01 | External stylesheet | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-14/C-03 | `E2E-CSS-01`; `E2E-CSS-03` | `PROJECTED` | `CLOSED` within supported subset |
| G7-02 | Inline `<style>` | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | A-14/C-03 | canonical coverage | `PROJECTED` | `CLOSED` within supported subset |
| G7-03 | Inline `style=""` declarations | `SUPPORTED / PROJECTED` within ordinary declarations and package-local URL boundary | XHTML source and ordinary declarations remain in reconstructed KF8 markup; inline style package-local URLs now reuse the existing CSS URL resolver and become reachable `kindle:embed` references | F-related | `characterization.rs::epub_css::css_characterization_matrix_observes_writer_transport`; `E2E-CSS-04` (`R5-01`..`R5-06`) | `PROJECTED` for ordinary declarations and quoted/unquoted package-local image/font URLs; external/data/literal URL text remains unchanged | `CLOSED` within characterized inline declaration/local URL boundary |
| G7-04 | General CSS parsing | `PARTIAL / TRANSPORT` | Limited semantic parser, source-preserving CSS flow, targeted Kindle projection, resource URL projection, and a known-unsupported safety scanner; not a browser-grade CSS evaluator | — | `E2E-CSS-03` plus CSS safety/projection suites | `PROJECTED` or `SAFELY REJECTED` by documented subcase; physical renderer evaluation is outside this audit | `CLOSED` within the defined converter CSS-processing architecture |
| G7-05 | Multiple/complex selectors | `PARTIAL / TRANSPORT + SAFE REJECT` | Selector transport is preserved where the converter does not transform it; `+`/`~` and documented unsupported pseudo families are handled by the CSS safety scanner | — | `E2E-CSS-03`; CSS false-positive and rejection suite | `PROJECTED` for transported selector subcases; `SAFELY REJECTED` for known KF8-unsupported subcases | `CLOSED` within the defined converter selector boundary |
| G7-06 | Descendant selectors | `PARTIAL / TRANSPORT` | `div p` survives in the final CSS flow and conversion succeeds; the E2E asserts the final CSS observable | — | `E2E-CSS-03` (`G7-06-descendant`) | `PROJECTED` within converter semantic transport scope; selector matching is outside this audit | `CLOSED` within converter semantic transport scope |
| G7-07 | Child/sibling combinators | `PARTIAL / TRANSPORT + SAFE REJECT` | `src/epub/css.rs` detects `+` and `~` in selector preludes while preserving the child `>` transport path | — | `E2E-CSS-03` (`G7-05-child`); CSS rejection and false-positive suites | `PROJECTED` for child `>` transport; `SAFELY REJECTED` for KF8-unsupported `E + F` / `E ~ F` | `CLOSED` within the defined converter selector boundary |
| G7-08 | Attribute selectors | `PARTIAL / TRANSPORT` | `[data-kind="x"]` survives unchanged in CSS flow and conversion succeeds; the E2E asserts the final CSS observable | — | `E2E-CSS-03` (`G7-08-attribute`) | `PROJECTED` within converter semantic transport scope; firmware matching is outside this audit | `CLOSED` within converter semantic transport scope |
| G7-09 | Pseudo-classes | `PARTIAL / SUPPORT + SAFE REJECT` | `src/epub/css.rs` preserves `:link` but rejects documented unsupported `:first-child` and functional `:nth-child(...)` in selector context | — | `E2E-CSS-02`; false-positive boundary suite | `PROJECTED` for `:link`; `SAFELY REJECTED` for the documented unsupported subcases | `CLOSED` within documented KF8 pseudo-class boundary |
| G7-10 | Pseudo-elements | `UNSUPPORTED / SAFE REJECT` | `src/epub/css.rs` rejects double-colon and legacy single-colon `before`, `after`, `first-letter`, and `first-line` selectors before KF8 output | — | `E2E-CSS-02` | `SAFELY REJECTED`; generated text cannot silently disappear into transported CSS | `CLOSED` |
| G7-11 | `@media` | `SUPPORTED / PROJECTED` within observed media-query subset | CSS flow retains `@media amzn-kf8` and `@media screen` wrappers and nested rules | — | `characterization.rs::epub_css::css_characterization_matrix_observes_writer_transport` (`CSS-10`, `CSS-11`) | `PROJECTED`; Amazon documents KF8/screen media-query support | `CLOSED` within the characterized transport boundary |
| G7-12 | `@supports` | `PARTIAL / TRANSPORT` | `@supports` prelude, wrapper, nested rule, and declarations survive CSS flow; the E2E asserts the final CSS observable | — | `E2E-CSS-03` (`G7-12-supports`) | `PROJECTED` at converter boundary; condition evaluation by Kindle firmware is outside this audit | `CLOSED` within converter semantic transport scope |
| G7-13 | `@font-face` | `SUPPORTED` within current embedded-font path | `CONFIRMED IMPLEMENTED` | E-12 | `E2E-FONT-01` | `PROJECTED` | `CLOSED` |
| G7-14 | `@import` | `SUPPORTED / PROJECTED` within local dependency graph | Local import, media-suffix import, and a cycle are traversed without looping; local targets are rewritten to `kindle:flow` references while preserving the import channel | A-14/C-03 related | `characterization.rs::epub_css::css_characterization_matrix_observes_writer_transport` (`CSS-13`..`CSS-15`) | `PROJECTED`; Amazon lists `@import` and documents KF8 media-query import usage | `CLOSED` within local-import/cycle boundary |
| G7-15 | CSS URLs | `SUPPORTED / PROJECTED` within external stylesheet and inline package-local URL boundary | External stylesheet and inline `style=""` package-local `url()` references use the same resolver and are rewritten to reachable `kindle:embed` references | A-14/E-related | `characterization.rs::epub_css::css_characterization_matrix_observes_writer_transport`; `E2E-CSS-04` (`R5-01`..`R5-06`) | `PROJECTED` for external and quoted/unquoted inline package-local image/font URLs; HTTPS and literal text remain unchanged; data URLs are preserved under the G7-16 transport boundary | `CLOSED` within characterized URL and data-URL transport boundaries |
| G7-16 | Data URLs in CSS | `PARTIAL / TRANSPORT` | PNG, JPEG, SVG, and generic `data:` URLs remain byte-for-byte observable in CSS flow; the converter does not misclassify them as package resources | — | `E2E-CSS-03` (`G7-16-data-*`) and URL boundary E2E | `PROJECTED` at converter boundary; firmware data-URL rendering is outside this audit | `CLOSED` within converter semantic transport scope |
| G7-17 | Writing mode | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-01/F-02/C-16 | `E2E-CANON-03`; `E2E-CSS-01` | `PROJECTED` | `CLOSED` |
| G7-18 | Direction / bidi presentation | `SUPPORTED / TRANSPORT` | `src/book/style.rs` parses CSS `direction`; generated KF8 layout CSS preserves the selected direction, while `unicode-bidi` remains in transported CSS source | F-01 related | `characterization.rs::epub_input_boundaries::language_bidi_and_direction_transport_is_observable` | `PROJECTED`: KF8 exposes consumed `direction` / `unicode-bidi` CSS semantics; renderer parity is outside this converter-level audit | `CLOSED` within semantic transport scope |
| G7-19 | Text orientation | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-02 | `E2E-CANON-03` | `PROJECTED` | `CLOSED` |
| G7-20 | TCY | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-02 | `E2E-CANON-03` | `PROJECTED` | `CLOSED` |
| G7-21 | Ruby presentation | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-02 | `E2E-CANON-03` | `PROJECTED` | `CLOSED` |
| G7-22 | Emphasis | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-03 | `E2E-CANON-03` | `PROJECTED` | `CLOSED` |
| G7-23 | Underline | `SUPPORTED` | `CONFIRMED IMPLEMENTED` | F-03 | `E2E-CANON-03` | `PROJECTED` | `CLOSED` |
| G7-24 | Page-break CSS | `SUPPORTED / PROJECTED` within characterized properties | `page-break-before`, `break-before`, and `page-break-inside` survive in CSS flow and conversion succeeds | page semantics related | `characterization.rs::epub_css::css_characterization_matrix_observes_writer_transport` (`CSS-19`..`CSS-21`) | `PROJECTED`; Amazon documents page-break-before/after/inside and break-before/after support | `CLOSED` within the characterized property/value boundary |
| G7-25 | CSS counters | `UNSUPPORTED / SAFE REJECT` | `src/epub/css.rs` rejects `counter-reset`, `counter-increment`, and active `counter(...)`/`counters(...)` functions outside strings and URLs | — | `E2E-CSS-02`; literal-value false-positive suite | `SAFELY REJECTED`; KF8-unsupported counter semantics cannot convert successfully | `CLOSED` |
| G7-26 | Generated content | `UNSUPPORTED / SAFE REJECT` | `src/epub/css.rs` rejects active generated `content:` values; `normal`/ `none` remain harmless, and pseudo-element rejection covers selector-generated content | — | `E2E-CSS-02`; generated-content and false-positive suites | `SAFELY REJECTED`; literal and counter-generated content cannot silently disappear | `CLOSED` |
| G7-27 | CSS custom properties | `PARTIAL / TRANSPORT` | Declaration-only custom properties and consumed `var()` syntax both survive the final CSS flow; the E2E asserts both subcases | — | `E2E-CSS-03` (`G7-27-declaration-only`, `G7-28-var-consumption`) | `PROJECTED` at converter boundary; no separate reader-side evaluation claim is made | `CLOSED` within converter semantic transport scope |
| G7-28 | Presentation-critical CSS variables | `PARTIAL / TRANSPORT` | `display: var(--display-mode)` and its declaration survive CSS flow and conversion | — | `E2E-CSS-03` (`G7-28-var-consumption`) | `PROJECTED` at converter boundary; firmware `var()` evaluation is outside this audit | `CLOSED` within converter semantic transport scope |
| G7-29 | General EPUB CSS outside canonical AozoraEpub3 | `PARTIAL / DEFINED BOUNDARY` | Limited parser, source-preserving transport, targeted projection, package-resource rewrite, and explicit known-unsupported scanner define the converter boundary; no browser-grade evaluator is claimed | — | CSS safety/projection suites plus `E2E-CSS-03` | Mixed: `PROJECTED / TRANSPORTED` for defined converter subcases and `SAFELY REJECTED` for known KF8-unsupported subcases | `CLOSED` within converter semantic transport and safe-rejection scope |

---

## G8 — Interactive / Extended EPUB

| ID | EPUB Feature | Coverage | Source Inspection | A–F Mapping | E2E Evidence | Safety | Closure |
|---|---|---|---|---|---|---|---|
| G8-01 | Scripted Content Documents | `OUT OF SCOPE / SAFE REJECT` | `src/epub/xhtml.rs::reject_scripting` rejects actual script/form semantics in spine XHTML and direct SVG sources before KF8 lowering or SVG wrapping | — | `E2E-SCRIPT-01`; `E2E-SCRIPT-02` | `SAFELY REJECTED`; no scripted document becomes a successful AZW3 | `CLOSED` within the explicit rejection boundary |
| G8-02 | Inline JavaScript | `OUT OF SCOPE / SAFE REJECT` | Inline executable script is detected in XHTML or direct SVG source during content ingestion | — | `E2E-SCRIPT-01`; `E2E-SCRIPT-02` | `SAFELY REJECTED` with a feature-specific error | `CLOSED` |
| G8-03 | External JavaScript | `OUT OF SCOPE / SAFE REJECT` | Script elements with `src` in XHTML or `href`/`xlink:href` in direct SVG are rejected before successful output | — | `E2E-SCRIPT-01`; `E2E-SCRIPT-02` | `SAFELY REJECTED` with a feature-specific error | `CLOSED` |
| G8-04 | Script fallback semantics | `OUT OF SCOPE / SAFE REJECT` | Fallback resolution reaches scripted XHTML, then applies the same explicit rejection boundary | fallback related | `E2E-SCRIPT-03` | `SAFELY REJECTED`; scripted content is not silently replaced or emitted as successful AZW3 | `CLOSED` |
| G8-05 | HTML forms | `OUT OF SCOPE / SAFE REJECT` | HTML `form` elements in spine XHTML or direct SVG source are rejected before KF8 lowering or SVG wrapping | — | `E2E-SCRIPT-01`; `E2E-SCRIPT-02` | `SAFELY REJECTED` with a feature-specific error | `CLOSED` |
| G8-06 | EPUB Media Overlays | `OUT OF SCOPE / SAFE REJECT` | `src/epub/opf.rs` retains manifest `media-overlay`; `src/epub/package.rs::reject_unsupported_media_semantics` rejects associated overlay input before output | — | `E2E-ACCESS-04` | `SAFELY REJECTED` for linked overlay semantics; unassociated SMIL is safely ignored | `CLOSED` |
| G8-07 | SMIL documents | `OUT OF SCOPE / SAFE REJECT` | Spine `application/smil+xml` is rejected; unlinked manifest-only SMIL is not added to reading order and is safely omitted | — | `E2E-ACCESS-04`; `E2E-ACCESS-05` | `SAFELY REJECTED` when spine/overlay-associated; safely ignored when unlinked | `CLOSED` |
| G8-08 | Media Overlay metadata | `INTENTIONALLY UNPROJECTED / SAFE IGNORE` | `media:*` metadata is not a KF8 reading semantic; association-bearing manifest links are rejected, while unassociated metadata does not enter content or navigation | — | `E2E-ACCESS-04` | `SAFELY IGNORED` only for unassociated declarative metadata; linked overlay semantics are `SAFELY REJECTED` | `CLOSED` |
| G8-09 | Synchronized audio/text | `OUT OF SCOPE / SAFE REJECT` | The current pipeline has no synchronized narration IR; audio/video playback and media-overlay associations reject before successful AZW3 output | — | `E2E-ACCESS-02`; `E2E-ACCESS-04` | `SAFELY REJECTED`; fallback text is not silently presented as equivalent synchronized playback | `CLOSED` |
| G8-10 | Accessibility metadata | `INTENTIONALLY UNPROJECTED / SAFE IGNORE` | Schema accessibility discoverability metadata remains outside the current KF8 metadata projection and is not injected into content/reading order | — | `E2E-ACCESS-01` | `SAFELY IGNORED` as declarative metadata; content and navigation remain unchanged | `CLOSED` |
| G8-11 | ARIA structural semantics | `PARTIAL / TRANSPORT` | Existing XHTML/RawML path preserves ARIA attributes without claiming assistive-technology evaluation | G3-13 | `E2E-CONTENT-01`; accessibility E2E | `PROJECTED` within converter semantic transport scope | `CLOSED` within converter semantic transport scope |
| G8-12 | Assistive reading-order semantics | `PARTIAL / TRANSPORT` | Spine order is lowered to KF8 sections and ARIA markers remain in source markup; assistive-reader interpretation is outside converter scope | G3-13 / navigation | `E2E-ACCESS-01` | `PROJECTED` for converter reading order and marker transport; renderer/assistive behavior is outside scope | `CLOSED` within converter semantic transport scope |
| G8-13 | Image alternative text | `PARTIAL / TRANSPORT` | `src/kf8/rawml.rs::rewrite_section_assets` rewrites image references without removing `alt`; XHTML source is otherwise preserved | navigation `alt` only partly evidenced | `characterization.rs::epub_input_boundaries::image_alt_and_figcaption_transport_is_observable` | `PROJECTED` for meaningful/empty/missing `alt` distinction and `figcaption` transport; assistive-reader behavior is outside this converter-level evidence | `CLOSED` within semantic transport scope |
| G8-14 | EPUB accessibility conformance metadata | `INTENTIONALLY UNPROJECTED / SAFE IGNORE` | `dcterms:conformsTo` and schema accessibility conformance/discoverability metadata are declarative publication metadata with no current KF8 projection; parser does not map them into content or navigation | — | `E2E-ACCESS-01` | `SAFELY IGNORED`; omission does not change reading content/navigation | `CLOSED` |

---

# Part II — KF8/AZW3 Output Contract

## Layer A — EPUB Source Contract

| ID | Contract | Evidence basis | Requirement status | Source inspection |
|---|---|---|---|---|
| A-01 | Preserve core package metadata required by the supported conversion path | `SPEC` | REQUIRED | `IMPLEMENTED` |
| A-02 | Metadata refinements not projected into KF8 are intentionally unprojected rather than accidentally lost | `SPEC + PRODUCT-CONTRACT` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| A-03 | Preserve manifest identity and required resource relationships | `SPEC` | REQUIRED | `IMPLEMENTED` |
| A-04 | Preserve spine order and `linear` semantics | `SPEC` | REQUIRED | `IMPLEMENTED` |
| A-05 | Preserve page progression direction and root writing-mode semantics | `SPEC` | REQUIRED | `IMPLEMENTED` |
| A-06 | Correctly interpret nav document manifest/spine semantics | `SPEC` | REQUIRED | `IMPLEMENTED` |
| A-07 | Preserve navigation hierarchy, labels, and destinations | `SPEC` | REQUIRED | `IMPLEMENTED` |
| A-08 | Preserve EPUB landmark semantics used by the supported path | `SPEC` | REQUIRED | `IMPLEMENTED` |
| A-09 | Correctly identify the EPUB cover-image resource | `SPEC + AMAZON-GUIDE` | REQUIRED | `IMPLEMENTED` |
| A-10 | Suppress a cover document from ordinary body content when represented as the native Kindle cover, except where source reading-order semantics require that linear pre-paginated cover document to remain a fixed-layout page | `SPEC + AMAZON-GUIDE + KINDLEGEN + PRODUCT-CONTRACT` | REQUIRED / EXTENDED WHEN APPLICABLE | `IMPLEMENTED; ORDINARY AND FULL FXL COMIC E2E VERIFIED` |
| A-11 | When the source cover document is suppressed as the native cover, keep its cover landmark tied to the native cover resource without retargeting it to the current navigation/TOC position; the cover resolves to the native cover resource while the TOC remains a nav-position target. | `SPEC + AMAZON-GUIDE + KINDLEGEN + PRODUCT-CONTRACT` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| A-12 | Keep title-page, bodymatter, text/start semantics distinct | `SPEC + AMAZON-GUIDE` | REQUIRED | `IMPLEMENTED` |
| A-13 | Preserve body hyperlinks and fragment targets | `SPEC` | REQUIRED | `IMPLEMENTED` |
| A-14 | Preserve the XHTML/CSS/image dependency graph required by rendered content | `SPEC` | REQUIRED | `IMPLEMENTED` |
| A-15 | Determine and retain the effective layout of every spine item, including item-level `rendition:layout` override | `SPEC` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| A-16 | Distinguish global layout from item-level layout classification without treating item-level overrides as proof of an Amazon-supported mixed-layout publication mode | `SPEC + CANONICAL-INPUT + KINDLEGEN` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| A-17a | Preserve publication-level `rendition:orientation` using the established Amazon orientation mapping and the self projection for fixed, mixed-layout, and reflowable publications; item-level overrides remain the separate G6-05 contract | `SPEC + AMAZON-GUIDE + PRODUCT-CONTRACT` | REQUIRED WHEN PRESENT | `IMPLEMENTED; E2E VERIFIED` |
| A-17b | Preserve Amazon fixed-layout publication metadata | `AMAZON-GUIDE` | REQUIRED IF PRESENT | `IMPLEMENTED` |
| A-17c | Preserve `original-resolution` | `AMAZON-GUIDE` | REQUIRED FOR APPLICABLE FXL | `IMPLEMENTED` |
| A-17d | Preserve `orientation-lock` | `AMAZON-GUIDE` | REQUIRED IF PRESENT | `IMPLEMENTED` |
| A-17e | Preserve explicit canonical AozoraEpub3 `book-type=\"comic\"` when present; do not synthesize other book types heuristically. Amazon `book-type=children` is recognized separately under KAMZ-01 and does not expand this canonical contract. | `AMAZON-GUIDE + CANONICAL-INPUT` | REQUIRED WHEN PRESENT IN CANONICAL INPUT | `IMPLEMENTED; EXPLICIT COMIC VERIFIED` |
| A-17f | Preserve `primary-writing-mode` where applicable | `AMAZON-GUIDE` | REQUIRED IF PRESENT | `IMPLEMENTED` |

Mixed-layout boundary: Amazon fixed-layout guidance states that a book cannot be
partially reflowable and partially fixed-layout. EPUB 3.3 semantics and
KindleGen/self compatibility for a canonical item-level pre-paginated lowering do
**not** establish that Amazon KDP supports a general mixed-layout publication model.
A-16 remains the explicit classification boundary; item-level source semantics may
be projected for the supported path while broader Amazon publication support remains
unclaimed.

---

## Layer B — MOBI/KF8 Header & Metadata

| ID | Contract | Evidence basis | Requirement status | Source inspection |
|---|---|---|---|---|
| B-01 | Valid PalmDB identity and record-table geometry; `BOOK` / `MOBI`; ordered, in-range records | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| B-02 | Valid PalmDOC header; supported compression mode; correct text length/count; ≤4096-byte uncompressed text records | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| B-03a | Standalone KF8 output declares MOBI version 8 | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| B-03b | Ordinary book output uses MOBI document type 2 | `REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED` |
| B-04 | MOBI text encoding is UTF-8 / 65001 and payload is consistent with that declaration | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| B-05a | PalmDOC text-record count and reconstructed uncompressed geometry agree | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| B-05b | Every emitted MOBI record pointer resolves to the intended record in this file | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| B-06 | First Image identifies the correct image-addressing base and relative image references resolve from it | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| B-07 | EXTH block magic, length, count, and individual record geometry are structurally valid | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| B-08 | Publication-level fixed-layout KF8 emits EXTH 122=`true`; item-level pre-paginated pages alone do not synthesize it | `REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| B-09 | Explicit canonical source `book-type=\"comic\"` lowers to EXTH 123=`comic` without heuristic synthesis | `AMAZON-GUIDE + REFERENCE + KINDLEGEN + CANONICAL-INPUT` | REQUIRED WHEN PRESENT IN CANONICAL INPUT | `IMPLEMENTED; CANONICAL COMIC VERIFIED` |
| B-10 | Applicable orientation-lock metadata lowers consistently to EXTH 124 | `AMAZON-GUIDE + REFERENCE + KINDLEGEN` | REQUIRED IF PRESENT | `IMPLEMENTED` |
| B-11 | EXTH 125 is not forced to KindleGen numeric parity until exact semantics are sufficiently established | `REFERENCE + KINDLEGEN` | DIAGNOSTIC / UNRESOLVED | `DIAGNOSTIC` |
| B-12 | Applicable `original-resolution` lowers consistently to EXTH 126 | `AMAZON-GUIDE + REFERENCE + KINDLEGEN` | REQUIRED FOR APPLICABLE FXL | `IMPLEMENTED` |
| B-13 | EXTH 127 zero-gutter behavior is not synthesized without evidence | `REFERENCE + KINDLEGEN` | OPTIONAL / COMPATIBILITY | `IMPLEMENTED; ABSENCE E2E VERIFIED` |
| B-14 | EXTH 128 zero-margin behavior is not synthesized without evidence | `REFERENCE + KINDLEGEN` | OPTIONAL / COMPATIBILITY | `IMPLEMENTED; ABSENCE E2E VERIFIED` |
| B-15 | EXTH 201 cover offset uses First-Image-relative semantics correctly | `REFERENCE` | REQUIRED IF COVER EXISTS | `IMPLEMENTED` |
| B-16 | EXTH 202 thumbnail offset uses First-Image-relative semantics correctly | `REFERENCE` | REQUIRED IF THUMBNAIL EXISTS | `IMPLEMENTED` |
| B-17 | EXTH 129 resource URI resolves to the intended KF8 thumbnail resource | `REFERENCE + KINDLEGEN` | REQUIRED WHEN EMITTED | `IMPLEMENTED` |
| B-18 | EXTH 116 Start Reading position resolves to the intended semantic start | `AMAZON-GUIDE + REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED` |
| B-19 | EXTH 525 preserves primary-writing-mode semantics where applicable | `AMAZON-GUIDE + REFERENCE + KINDLEGEN` | REQUIRED WHEN APPLICABLE | `IMPLEMENTED` |
| B-20 | EXTH 527 preserves page-progression direction where applicable | `SPEC + REFERENCE + KINDLEGEN` | REQUIRED WHEN APPLICABLE | `IMPLEMENTED` |
| B-21 | Extra-data flags agree with actual trailing-data serialization | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| B-22 | Optional/vendor fields are generated only when supported by evidence | `REFERENCE + PRODUCT-CONTRACT` | DO NOT GUESS | `IMPLEMENTED; REVIEWED` |

## B-layer notes

KindleGen numeric record positions are not required to match self output when
record geometry legitimately differs.

Pointers must instead resolve correctly within the generated artifact.

EXTH 125 is explicitly not a repair target at this time.

---

## Layer C — KF8 Content Lowering

| ID | Contract | Evidence basis | Requirement status | Source inspection |
|---|---|---|---|---|
| C-01 | Supported spine XHTML is lowered into valid KF8 content | `SPEC + REFERENCE` | REQUIRED | `IMPLEMENTED` |
| C-02 | Main reflowable HTML content occupies the correct primary RawML flow | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| C-03 | CSS content required by the book is represented by valid secondary flows | `REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED` |
| C-04 | Secondary-flow types other than the currently exercised CSS and fixed-page SVG flows are represented correctly when a supported input actually requires them | `REFERENCE + KINDLEGEN` | CONDITIONAL / NON-CANONICAL UNTIL EXERCISED | `CONDITIONAL-GAP` |
| C-05 | Item-level pre-paginated spine items are recognized during lowering | `SPEC + CANONICAL-INPUT` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| C-06 | A pre-paginated fixed page is separated from main RawML when the canonical KindleGen-compatible lowering requires it | `KINDLEGEN + REFERENCE` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| C-07 | Such page SVG is emitted as an independent secondary flow | `KINDLEGEN + REFERENCE` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| C-08 | Main RawML references the page flow through the appropriate `kindle:flow:` URI | `KINDLEGEN + REFERENCE` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| C-09 | FDST describes every semantically real RawML flow required by the generated book | `REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| C-10 | FDST flow ranges are valid, ordered, non-overlapping, and within RawML | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| C-11 | Secondary-flow URI and MIME semantics resolve to the intended flow type | `REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| C-12 | Canonical input containing reflowable content plus item-level pre-paginated pages preserves both semantics during KF8 lowering | `SPEC + CANONICAL-INPUT + KINDLEGEN` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| C-13 | Full fixed-layout/image-only publications preserve the semantically real fixed-page flow set, including a linear pre-paginated cover page when same-input/source semantics require it | `AMAZON-GUIDE + REFERENCE + KINDLEGEN` | EXTENDED | `IMPLEMENTED; STRUCTURAL E2E AND SAME-INPUT COVER PARITY VERIFIED` |
| C-14 | Ordered-list ordinal semantics are materialized into explicit `li@value` during KF8 lowering. Nested ordered lists maintain independent counters, and existing `ol@start` / `li@value` semantics are preserved when computing subsequent ordinal values. | `SPEC + KINDLEGEN + REFERENCE + PRODUCT-CONTRACT` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| C-15 | Supported `span` elements participating in KF8 RawML lowering receive AID identifiers consistently with the KindleGen-compatible aid-bearing element set. | `KINDLEGEN + REFERENCE + PRODUCT-CONTRACT` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| C-16 | Mixed writing-mode lowering preserves the source XHTML DOM structure. A horizontal/vertical writing-mode transition already expressed by source `hltr`/`vrtl` semantics must not be implemented by inserting synthetic CSS-active wrapper elements that alter selector matching, inheritance, or layout semantics. | `SPEC + KINDLEGEN + REFERENCE + PRODUCT-CONTRACT` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |

---

## Layer D — KF8 Index / Navigation / Position Geometry

| ID | Contract | Evidence basis | Requirement status | Source inspection |
|---|---|---|---|---|
| D-01 | SKEL structure and entries are valid | `REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED` |
| D-02 | FRAG structure and entries are valid | `REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED` |
| D-03 | SKEL/FRAG routing reconstructs intended content correctly | `REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED` |
| D-04 | INDX headers, entries, and tables are valid | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| D-05 | CNCX strings and offsets resolve correctly | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| D-06 | KF8 NCX/index representation preserves source navigation semantics | `SPEC + REFERENCE` | REQUIRED | `IMPLEMENTED` |
| D-07 | Guide entries preserve applicable Kindle navigation semantics | `AMAZON-GUIDE + REFERENCE` | REQUIRED WHEN APPLICABLE | `IMPLEMENTED` |
| D-08 | Start Reading / bodymatter position geometry resolves to intended RawML position | `AMAZON-GUIDE + REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED` |
| D-09 | PositionMap is structurally and semantically correct when generated | `REFERENCE + KINDLEGEN` | REQUIRED WHEN GENERATED | `IMPLEMENTED` |
| D-10 | TBS data is structurally consistent with associated text/index geometry | `REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED` |
| D-11 | FCIS uses the evidenced KindleGen/calibre-compatible canonical 52-byte KF8 shape; field @12 and record length are not derived from FDST flow count, and the exact semantics of field @12 are undocumented. FCIS pointers remain valid. | `REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| D-12 | FLIS structure and pointers are valid | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| D-13 | DATP is tracked diagnostically; its generation requirement and payload semantics are not yet established strongly enough for mandatory parity | `REFERENCE + KINDLEGEN` | DIAGNOSTIC / UNRESOLVED | `DIAGNOSTIC / ABSENT` |
| D-14 | Trailing-data serialization matches declared extra-data flags | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| D-15 | Record boundaries, ordering, and referenced ranges remain valid | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| D-16 | Multi-detail INDX geometry remains valid after detail-count thresholds are crossed | `REFERENCE + PRODUCT-CONTRACT` | REQUIRED WHEN EXERCISED | `IMPLEMENTED; E2E VERIFIED` |

`RESC` in the rows below is a KindleGen-final observed serialization label and a
reference interpretation, not an Amazon-specified record name. Amazon evidence supplies
reader-visible/source semantics only; EPUB evidence supplies source order and identity;
KINDLEGEN-FINAL supplies observed RESC serialization; and REFERENCE interprets the
resulting KF8 geometry. The contracts below are closed only for the demonstrated
self-writer projection and E2E scope; they do not promote Amazon semantic guidance
into a binary-format claim.

| D-17 | RESC geometry is valid for the serialized resource/spine-property entries and remains internally addressable | `SPEC + AMAZON-GUIDE + KINDLEGEN-FINAL + REFERENCE` | IMPLEMENTED; E2E VERIFIED | `E2E-RESC-03` validates RESC magic/prefix, variable-width size field, UTF-8 XML, 4KB-block padding, record range, resource ordering, and FLIS placement. Amazon evidence remains semantic only. |
| D-18 | RESC entries preserve EPUB spine order where the source semantics are supported | `SPEC + AMAZON-GUIDE + KINDLEGEN-FINAL + REFERENCE` | IMPLEMENTED; E2E VERIFIED | The RESC E2E fixture compares surviving source spine order with itemref order; synthetic TOC/page-list sections are excluded from the source projection. |
| D-19 | Each RESC idref retains identity with the intended EPUB spine/manifest item | `SPEC + AMAZON-GUIDE + KINDLEGEN-FINAL + REFERENCE` | IMPLEMENTED; E2E VERIFIED | The RESC E2E fixture asserts `left`, `right`, and `center` source idrefs are retained unchanged. |
| D-20 | Every RESC skelid resolves to the intended SKEL entry | `SPEC + AMAZON-GUIDE + KINDLEGEN-FINAL + REFERENCE` | IMPLEMENTED; E2E VERIFIED | The RESC E2E fixture resolves every numeric skelid through reconstructed SKEL/FRAG XHTML and checks the section sentinel. |
| D-21 | Supported EPUB spine properties survive into the applicable final serialization without being silently dropped | `SPEC + AMAZON-GUIDE + KINDLEGEN-FINAL + REFERENCE` | IMPLEMENTED; E2E VERIFIED | The RESC E2E fixtures preserve `rendition:orientation-*`, `rendition:spread-*`, `rendition:flow-*`, `rendition:align-x-center`, and `rendition:page-spread-left/right/center` in source spelling and order; aliases are parsed into typed semantics while the original itemref property vector feeds RESC. |
| D-22 | Suppressed native-cover XHTML leaves RESC/SKEL identity and omission/remapping consistent with the surviving cover/resource semantics | `SPEC + AMAZON-GUIDE + KINDLEGEN-FINAL + REFERENCE` | IMPLEMENTED; E2E VERIFIED | The RESC E2E fixture confirms suppressed native-cover XHTML has neither a RESC itemref nor a surviving SKEL XHTML entry, while the native cover resource remains in resource geometry. |

---

## Layer E — KF8 Resource Geometry

| ID | Contract | Evidence basis | Requirement status | Source inspection |
|---|---|---|---|---|
| E-01 | First Image and image-resource geometry are internally correct | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| E-02 | Image-resource ordering remains internally consistent with references | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| E-03 | Every `kindle:embed:` image reference resolves to the intended resource | `REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED` |
| E-04 | Native Kindle cover resource represents the intended EPUB cover | `AMAZON-GUIDE + REFERENCE` | REQUIRED | `IMPLEMENTED` |
| E-05 | Thumbnail resource is valid when generated | `REFERENCE` | REQUIRED WHEN GENERATED | `IMPLEMENTED` |
| E-06 | EXTH 201 ultimately resolves to the intended cover resource | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| E-07 | EXTH 202 ultimately resolves to the intended thumbnail resource | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| E-08 | EXTH 129 resolves to the intended KF8 thumbnail resource URI | `REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED` |
| E-09 | Cover and thumbnail remain semantically distinct resources when both exist | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| E-10 | Thumbnail bytes/dimensions may differ from cover when semantic identity remains correct | `REFERENCE + PRODUCT-CONTRACT` | ALLOWED | `IMPLEMENTED` |
| E-11 | Resource counts and resource-related pointers agree with actual serialized output | `REFERENCE` | REQUIRED | `IMPLEMENTED` |
| E-12 | Embedded FONT resource path is valid when an AozoraEpub3 input contains an embedded font referenced by `@font-face` | `REFERENCE + CANONICAL-INPUT` | REQUIRED WHEN PRESENT / CONDITIONAL CANONICAL FEATURE | `IMPLEMENTED; E2E VERIFIED` |
| E-13 | Generic non-image resource addressing beyond the dedicated FONT case is correct when exercised | `REFERENCE` | CONDITIONAL / NON-CANONICAL UNTIL EXERCISED | `CONDITIONAL-GAP` |
| E-14 | SVG/page-flow references resolve to the intended secondary flow/resource topology | `REFERENCE + KINDLEGEN` | REQUIRED FOR PRE-PAGINATED LOWERING | `IMPLEMENTED; E2E VERIFIED` |

---

## Layer F — EPUB → AZW3 Semantic Preservation

Presentation semantics are audited for preservation through conversion.

This layer does **not** assert that every Kindle device renders every CSS or
SVG feature identically to an EPUB reading system.

| ID | Contract | Evidence basis | Requirement status | Source inspection |
|---|---|---|---|---|
| F-01 | Preserve writing direction and title-door semantics | `SPEC + AMAZON-GUIDE + PRODUCT-CONTRACT` | REQUIRED | `IMPLEMENTED` |
| F-02 | Preserve ruby, tcy, and upright-orientation semantics | `SPEC + PRODUCT-CONTRACT` | REQUIRED | `IMPLEMENTED` |
| F-03 | Preserve emphasis and underline semantics rather than silently dropping or reclassifying them | `SPEC + PRODUCT-CONTRACT` | REQUIRED | `IMPLEMENTED` |
| F-04 | Preserve canonical warichu semantics | `CANONICAL-INPUT + PRODUCT-CONTRACT` | REQUIRED | `IMPLEMENTED` |
| F-05 | Preserve source Unicode scalar sequence, including IVS, combining marks, and supplementary-plane characters | `PRODUCT-CONTRACT` | REQUIRED | `IMPLEMENTED` |
| F-06 | Preserve page/section order and reading order | `SPEC` | REQUIRED | `IMPLEMENTED` |
| F-07 | Preserve heading and block-level semantic structure required by the supported path | `SPEC + PRODUCT-CONTRACT` | REQUIRED | `IMPLEMENTED` |
| F-08 | Preserve images occurring in reflowable content | `SPEC + REFERENCE` | REQUIRED | `IMPLEMENTED` |
| F-09 | Preserve item-level pre-paginated source semantics through the KF8 lowering topology | `SPEC + CANONICAL-INPUT + KINDLEGEN` | REQUIRED | `IMPLEMENTED; E2E VERIFIED` |
| F-10 | Preserve visible table-of-contents semantics | `SPEC + AMAZON-GUIDE` | REQUIRED | `IMPLEMENTED` |
| F-11 | Preserve navigation/NCX labels and targets | `SPEC + REFERENCE` | REQUIRED | `IMPLEMENTED` |
| F-12 | Preserve bodymatter / start-reading semantics | `AMAZON-GUIDE + REFERENCE + KINDLEGEN` | REQUIRED | `IMPLEMENTED` |
| F-13 | Preserve title and creator metadata required by reader-visible book identity | `SPEC + AMAZON-GUIDE` | REQUIRED | `IMPLEMENTED` |
| F-14 | Preserve supported body hyperlinks and destinations | `SPEC + AMAZON-GUIDE` | REQUIRED | `IMPLEMENTED` |

Full image-only fixed-layout acceptance is maintained under Extended Scope
rather than as F-15 mandatory canonical acceptance. The final artifacts remain
outside mandatory canonical acceptance. Source-reading-order parity is IMPLEMENTED
for the retained-versus-suppressed fixed-layout cover page, with structural E2E and
same-input cover-parity verification.

---

# Part III — Kindle CSS Compatibility

## CSS compatibility model

The CSS audit keeps these dimensions separate:

```text
Amazon reader support
≠ KindleGen writer behavior
≠ required converter action
```

`Amazon = NO` never mechanically implies `REMOVE` or `REJECT`. The converter action is
chosen only after source semantics, current Amazon guidance, same-input writer behavior,
and semantic-loss risk are understood.

Closure/action vocabulary:

- `KEEP` — current writer behavior is acceptable.
- `SAFE-REMOVE` — removal is established as appropriate for the target.
- `SAFE-REJECT` — explicit rejection prevents silent semantic loss.
- `MATCH-KINDLEGEN` — demonstrated writer difference should be reconciled unless stronger evidence justifies self behavior.
- `CHARACTERIZE` — more evidence is required.
- `CLOSED` — current contract/evidence is sufficient.

## Compatibility matrix

### Declarations / properties

| ID | CSS construct | Amazon reader support | KindleGen diagnostic | KindleGen final writer | Self final writer | Same-input parity | Semantic-loss risk | Converter action | Closure |
|---|---|---:|---|---|---|---|---|---|---|
| KCSS-01 | `max-width` declaration | `NO` | `W28001` | `REMOVE` | `REMOVE` | `MATCH` | Reader support is marked No, but removing the declaration matches KindleGen's final writer behavior | `SAFE-REMOVE` | `CLOSED` |
| KCSS-02 | `max-height` declaration | `NO` | `W28001` | `REMOVE` | `REMOVE` | `MATCH` | Same property-level writer behavior; supported neighboring declarations remain preserved | `SAFE-REMOVE` | `CLOSED` |
| KCSS-03 | `outline` | `NO` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Reader support is marked No, but writer transport loses no source declaration | `KEEP` | `CLOSED` |
| KCSS-04 | `outline-color` | `NO` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Same: possible reader-side no-op, no writer-side loss | `KEEP` | `CLOSED` |
| KCSS-05 | `outline-style` | `NO` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Same: possible reader-side no-op, no writer-side loss | `KEEP` | `CLOSED` |
| KCSS-06 | `outline-width` | `NO` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Same: possible reader-side no-op, no writer-side loss | `KEEP` | `CLOSED` |
| KCSS-07 | `counter-reset` | `NO` | `NOT EXERCISED` | `NOT EXERCISED` | `REJECT` | `NOT ESTABLISHED` | Successful transport could silently lose generated numbering semantics | `SAFE-REJECT` | `CLOSED` via G7-25 |
| KCSS-08 | `counter-increment` | `NO` | `NOT EXERCISED` | `NOT EXERCISED` | `REJECT` | `NOT ESTABLISHED` | Same generated-numbering risk | `SAFE-REJECT` | `CLOSED` via G7-25 |
| KCSS-09 | active generated `content:` / counter-generated text | `NOT ESTABLISHED` | `NOT EXERCISED` | `NOT EXERCISED` | `REJECT` | `NOT ESTABLISHED` | Active generated content can silently remove visible content | `SAFE-REJECT` | `CLOSED` via G7-26 |


### Selectors / pseudo-classes

| ID | CSS construct | Amazon reader support | KindleGen diagnostic | KindleGen final writer | Self final writer | Same-input parity | Semantic-loss risk | Converter action | Closure |
|---|---|---:|---|---|---|---|---|---|---|
| KCSS-10 | adjacent sibling `E + F` | `NO` | `NOT EXERCISED` | `NOT EXERCISED` | `REJECT` | `NOT ESTABLISHED` | Selector semantics are not safely evaluable by the current converter | `SAFE-REJECT` | `CLOSED` via G7 safety contract |
| KCSS-11 | general sibling `E ~ F` | `NO` | `NOT EXERCISED` | `NOT EXERCISED` | `REJECT` | `NOT ESTABLISHED` | Same selector-evaluation risk | `SAFE-REJECT` | `CLOSED` via G7 safety contract |
| KCSS-12 | `:first-child` | `NO` | `none` | `PRESERVE` | `REJECT` | `DIFFERENCE` | Self rejection intentionally prevents silent selector loss; KindleGen preservation is not proof of reader support | `SAFE-REJECT` | `CLOSED` safety contract |
| KCSS-13 | `:nth-child(...)` | `NO` | `none` | `PRESERVE` | `REJECT` | `DIFFERENCE` | Same; existing rejection remains intentional | `SAFE-REJECT` | `CLOSED` safety contract |
| KCSS-14 | `:first-of-type` | `NO` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Reader may ignore the selector, but neither writer loses it | `KEEP` | `CLOSED` |
| KCSS-15 | `:last-child` | `NO` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Same reader-side risk only | `KEEP` | `CLOSED` |
| KCSS-16 | `:last-of-type` | `NO` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Same reader-side risk only | `KEEP` | `CLOSED` |
| KCSS-17 | `:nth-last-child(...)` | `NO` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Same reader-side risk only | `KEEP` | `CLOSED` |
| KCSS-18 | `:nth-last-of-type(...)` | `NO` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Same reader-side risk only | `KEEP` | `CLOSED` |
| KCSS-19 | `:nth-of-type(...)` | `NO` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Same reader-side risk only | `KEEP` | `CLOSED` |
| KCSS-20 | `:only-child` | `NO` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Same reader-side risk only | `KEEP` | `CLOSED` |
| KCSS-21 | `:only-of-type` | `NO` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Same reader-side risk only | `KEEP` | `CLOSED` |
| KCSS-22 | `:visited` | `NO` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Reader behavior remains outside converter guarantee; writer transport is lossless | `KEEP` | `CLOSED` |

### `:visited` control

`:visited` proves that unsupported reader semantics may remain in a valid
KindleGen-written KF8 CSS flow.

Therefore the characterized pseudo-class rows remain transportable rather than
being changed to blanket rejection merely because Appendix C says `No`.
The two explicit safe-reject controls remain separate because self conversion
would otherwise silently lose selector semantics.

---

### Pseudo-elements / generated selector semantics

| ID | CSS construct | Amazon reader support | Self behavior | Current action | Closure |
|---|---|---:|---|---|---|
| KCSS-23 | `::before` / legacy `:before` | `NO` | explicit reject | `KEEP` existing semantic-safety boundary | `CLOSED` |
| KCSS-24 | `::after` / legacy `:after` | `NO` | explicit reject | `KEEP` | `CLOSED` |
| KCSS-25 | `::first-letter` / legacy form | `NO` | explicit reject | `KEEP` | `CLOSED` |
| KCSS-26 | `::first-line` / legacy form | `NO` | explicit reject | `KEEP` | `CLOSED` |

These rows remain separate from harmlessly transportable unsupported selectors
because generated/selector-driven semantics can cause silent visible-content
or presentation loss.

---

### Supported-neighbor controls

Compatibility hardening must prove that neighboring supported constructs are
not removed accidentally.

| ID | CSS construct | Amazon reader support | KindleGen diagnostic | KindleGen final writer | Self final writer | Same-input parity | Semantic-loss risk | Converter action | Closure |
|---|---|---:|---|---|---|---|---|---|---|
| KCSS-27 | `width` | `YES` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Supported neighbor must not be removed by max-width handling | `KEEP` | `CLOSED` |
| KCSS-28 | `height` | `YES` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Same | `KEEP` | `CLOSED` |
| KCSS-29 | `min-width` | `YES` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Same | `KEEP` | `CLOSED` |
| KCSS-30 | `min-height` | `YES` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Same | `KEEP` | `CLOSED` |
| KCSS-31 | `outline-offset` | `YES` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Must remain independent from unsupported `outline*` siblings | `KEEP` | `CLOSED` |
| KCSS-32 | `position` | `YES` / value-contextual | `W28003` | `PRESERVE` | `PRESERVE` | `MATCH` | Warning does not establish semantic loss; value/context restrictions remain separate | `KEEP` | `CLOSED` |
| KCSS-33 | `:link` | `YES` | `none` | `PRESERVE` | `PRESERVE` | `MATCH` | Supported link state must not follow the `:visited` reader status mechanically | `KEEP` | `CLOSED` |

---

### `position` boundary

Canonical KindleGen diagnostics contain W28003 for:

```text
position: relative
position: absolute
```

but final same-input KF8 retains the relevant position semantics.

Therefore:

```text
warning
≠ removal
```

Do not implement:

```text
position → global remove
```

The audit must distinguish:

```text
property support
value/context restrictions
reflowable use
fixed-layout use
diagnostic behavior
final writer behavior
```

Any normalization must be context-specific and independently evidenced.

---

### At-rule boundary

The canonical KindleGen diagnostic says that this implementation supports:

```text
@import
@charset
@font-face
```

That diagnostic must not be treated as a complete modern normative list.

Existing stronger contracts already cover:

| ID | At-rule | Amazon reader support | KindleGen diagnostic | KindleGen final writer | Self final writer | Same-input parity | Semantic-loss risk | Converter action | Closure |
|---|---|---:|---|---|---|---|---|---|---|
| KCSS-34 | `@charset` | `YES` | `I10004` | `PRESERVE` | `PRESERVE` | `MATCH` | Encoding semantics remain an ingestion/serialization concern, not a reason to remove the rule | `KEEP` | `CLOSED` |
| KCSS-35 | `@font-face` | `YES` | `I10004` | `PRESERVE` | `PRESERVE` | `MATCH` | Font use still depends on packaged resource validity; diagnostic alone does not prove loss | `KEEP` | `CLOSED` |
| KCSS-36 | `@import` | `YES` | `I10004` | `PRESERVE` | `PRESERVE` | `MATCH` | Local dependency resolution remains separate from the at-rule's presence | `KEEP` | `CLOSED` |
| KCSS-37 | `@media` | `YES` for documented KF8 media-query usage | `I10004` | `PRESERVE` | `PRESERVE` | `MATCH` | Query application is reader/context-dependent, but writer transport matches | `KEEP` | `CLOSED` |
| KCSS-38 | `@page` | `NOT ESTABLISHED` | `I10004` | `PRESERVE` | `PRESERVE` | `MATCH` | Reader application is not established; no writer-side removal was measured | `KEEP` | `CLOSED` for writer parity |
| KCSS-39 | `@namespace` | `NOT ESTABLISHED` | `I10004` | `PRESERVE` | `PRESERVE` | `MATCH` | Reader application is not established; no writer-side removal was measured | `KEEP` | `CLOSED` for writer parity |
| KCSS-40 | `@supports` | `NOT ESTABLISHED` | `I10004` | `PRESERVE` | `PRESERVE` | `MATCH` | Reader application is not established; no writer-side removal was measured | `KEEP` | `CLOSED` for writer parity |

Absence from an Appendix table must not automatically be treated as `NO`.

---

# Part IV — Product Scope and Known Boundaries

## Product scope

Full image-only fixed-layout publications are classified as **Extended Scope**.
They are valuable diagnostic inputs but are not by themselves part of the mandatory
canonical release-acceptance path. An Extended-Scope artifact must nevertheless be
investigated when it exposes a defect that also reproduces in canonical AozoraEpub3
input.

The evidenced publication-level fixed-layout comic path preserves a linear
pre-paginated cover document as a fixed page when source reading-order semantics
require it, while the same image may also serve as the native Kindle cover.

## Intentionally diagnostic / unresolved

The following are documented but are not release repair targets without stronger
evidence:

```text
B-11 — EXTH 125 exact resource-count semantics
D-13 — DATP generation requirement and payload semantics
```

EXTH 125 is not forced to KindleGen numeric parity. DATP must not be added merely
to imitate KindleGen; the current output intentionally leaves its pointer at the
null sentinel when DATP is absent.

## Conditional / non-canonical gaps

```text
C-04 — secondary-flow types beyond the currently exercised CSS/fixed-page SVG set
E-13 — generic non-image resource addressing beyond the dedicated FONT case
```

These do not block the current canonical release path unless a supported input
actually exercises them.

---

# Part V — Physical Kindle Evidence

## Physical Kindle evidence

Physical Kindle validation is the final acceptance oracle for reader-visible
behavior. It may establish that the generated AZW3 opens successfully, Start Reading
reaches the intended content, navigation destinations are reachable, generated
content/pages are reachable, and a serialization defect causes a reader-visible
failure.

Physical Kindle evidence is not treated as a CSS property support table, SVG
capability matrix, pixel-perfect visual-fidelity requirement, or per-feature renderer
benchmark. The converter's responsibility is semantic preservation.

## Layer L — Physical Kindle Device Evidence

This evidence is based on a representative final candidate plus the broad
the canonical Physical Kindle diagnostic run. It does not claim 13 dedicated
fixtures/manual tests, complete Kindle CSS support, complete renderer compatibility,
or formal image-only fixed-layout support.

| ID | Contract | Conversion Coverage | Classification | Physical Kindle Device Evidence |
|---|---|---|---|---|
| L-01 | Writing direction and mixed/title-door lowering | `COVERED` | `SEMANTIC EQUIVALENT` | `CONFIRMED PASS` |
| L-02 | Inline annotation/orientation preservation | `COVERED` | `SEMANTIC EQUIVALENT` | `CONFIRMED PASS` |
| L-03 | Emphasis/underline semantic preservation | `COVERED` | `SEMANTIC EQUIVALENT` | `CONFIRMED PASS` |
| L-04 | Warichu preservation | `COVERED` | `SEMANTIC EQUIVALENT` | `CONFIRMED PASS` |
| L-05 | Text/glyph fidelity | `COVERED` | `SEMANTIC EQUIVALENT` | `CONFIRMED PASS` |
| L-06 | Page/section boundary preservation | `COVERED` | `SEMANTIC EQUIVALENT` | `CONFIRMED PASS` |
| L-07 | Heading/block structure preservation | `COVERED` | `SEMANTIC EQUIVALENT` | `CONFIRMED PASS` |
| L-08 | Image/resource presentation path preservation | `COVERED` | `SEMANTIC EQUIVALENT` | `CONFIRMED PASS` |
| L-09 | Visible TOC existence and reading-order placement | `COVERED` | `SEMANTIC EQUIVALENT` | `CONFIRMED PASS` |
| L-10 | TOC/NCX navigation and labels | `COVERED` | `SEMANTIC EQUIVALENT` | `CONFIRMED PASS` |
| L-11 | Start Reading/bodymatter target preservation | `COVERED` | `SEMANTIC EQUIVALENT` | `CONFIRMED PASS` |
| L-12 | Title/creator semantics | `COVERED` | `SEMANTIC EQUIVALENT` | `CONFIRMED PASS` |
| L-13 | Body hyperlink preservation | `COVERED` | `SEMANTIC EQUIVALENT` | `CONFIRMED PASS` |

Formal Layer L summary:

```text
EPUB→AZW3 semantic contracts = 13 / 13 COVERED
SEMANTIC EQUIVALENT = 13
Physical Kindle Device Evidence = 13 / 13 CONFIRMED PASS
Known Physical Kindle failures attributable to self conversion = 0
```


## Supplemental device observation — Amazon ASIN cover fetch

The following is a supplemental Physical Kindle observation only. It is not
part of the mandatory A–F semantic contract, Layer L 13/13 acceptance count,
or release acceptance criteria.

On a Kindle Oasis, a sideloaded KF8/AZW3 containing:

```text
EXTH 113 = a valid Amazon Kindle ASIN
EXTH 501 = EBOK
EXTH 504 = the same ASIN
```

displayed the Amazon product's official library cover after Wi-Fi was enabled.

For the diagnostic run, the AZW3 body content was the canonical reference publication,
while the embedded ASIN identified an unrelated Amazon Kindle title. The
library cover changed to that unrelated title's Amazon cover, providing device
evidence that the displayed library thumbnail was obtained through Amazon ASIN
metadata/cover lookup rather than from the AZW3's embedded cover resource.

This observation is retained for diagnostic/reference purposes only. The
converter does not currently require, synthesize, or guarantee Amazon ASIN
metadata for library-cover behavior.

---

# Part VI — E2E Traceability

## Traceability rules

This audit is the authoritative mapping source for stable audit contracts and primary executable evidence.

- Audit IDs (`G`, `A`–`F`, `KCSS`, `KLAY`, `KAMZ`, `L`) are stable contract identifiers.
- Primary executable evidence uses stable `E2E-<DOMAIN>-NN` identifiers.
- E2E IDs identify scenarios; Rust function names may change without renumbering the audit contract.
- E2E source should carry the corresponding E2E ID as a module/test-adjacent comment or annotation.
- Test logic, assertions, and fixtures are independent of the identifier itself.
- Characterization/reference comparisons are supporting evidence and do not receive primary E2E IDs unless they become release-contract owners.
- Layer L and supplemental device observations remain device evidence rather than executable E2E ownership.

## Primary E2E mapping

| E2E ID | Scenario | Current test function / suite | Primary Audit IDs |
|---|---|---|---|
| `E2E-OCF-01` | Default rootfile / multiple-rendition boundary | `tests/e2e.rs::ocf_container::multiple_rootfiles_use_the_default_record_selection_boundary` | G1-01, G1-02, G1-03 |
| `E2E-OCF-02` | OCF mimetype non-semantic boundary | `tests/e2e.rs::ocf_container::mimetype_shapes_remain_outside_publication_semantics` | G1-02, G1-04 |
| `E2E-OCF-03` | Ancillary OCF metadata isolation | `tests/e2e.rs::ocf_container::ocf_companion_metadata_is_ignored_without_body_loss` | G1-01, G1-08, G1-09, G1-10, G1-11 |
| `E2E-OCF-04` | Local OCF href/path resolution | `tests/e2e.rs::resource_resolution::ocf_href_matrix_resolves_local_zip_names` | G1-05 |
| `E2E-OCF-05` | Unknown manifest-property coexistence | `tests/e2e.rs::ocf_container::generic_manifest_properties_preserve_feature_channels` | G2-20 |
| `E2E-OCF-06` | Package vs CSS data-URL boundary | `tests/e2e.rs::ocf_container::package_and_css_data_url_boundaries_are_distinct` | G2-17 |
| `E2E-OCF-07` | Package collection omission boundary | `tests/e2e.rs::ocf_container::package_collection_shapes_record_the_omission_boundary` | G2-18 |
| `E2E-OCF-08` | Package link metadata isolation | `tests/e2e.rs::ocf_container::package_link_metadata_is_not_a_resource` | G2-12 |
| `E2E-OCF-09` | Legacy guide does not override EPUB 3 navigation | `tests/e2e.rs::ocf_container::legacy_guide_does_not_override_epub3_navigation` | G2-19 |
| `E2E-META-01` | Dublin Core projection and safe omission | `tests/e2e.rs::resource_resolution::dublin_core_projection_and_safe_omission` | G2-06 |
| `E2E-META-02` | Metadata refinements and collection isolation | `tests/e2e.rs::package_metadata::metadata_refinements_project_without_collection_leakage` | G2-07, G2-08, G2-09, G2-10, G2-11; A-02 |
| `E2E-META-03` | Manifest fallback-chain projection | `tests/e2e.rs::package_metadata::fallback_chain_projects_content_and_rejects_malformed_chains` | G2-13, G2-14 |
| `E2E-META-04` | Navigation channel separation and page targets | `tests/e2e.rs::package_metadata::navigation_channels_and_page_targets_are_separate` | G5-09, G5-10, G5-12, G5-16 |
| `E2E-META-05` | Direct SVG content-document lowering | `tests/e2e.rs::package_metadata::svg_content_document_is_lowered_into_kf8_flow` | G3-05 |
| `E2E-CONTENT-01` | General XHTML structural/attribute transport | `tests/e2e.rs::content_documents::general_xhtml_semantics_project_without_attribute_loss` | G3-02, G3-03, G3-08, G3-09, G3-13, G3-20, G8-11 |
| `E2E-CONTENT-02` | Internal/cross-document/external link handling | `tests/e2e.rs::content_documents::cross_document_and_external_links_are_safe` | G3-14, G3-15, G3-16, G3-17 |
| `E2E-CONTENT-03` | Picture fallback and package-local srcset safety | `tests/e2e.rs::content_documents::picture_fallback_projects_and_package_srcset_rejects` | G3-19 |
| `E2E-CONTENT-04` | Custom/unlinked navigation hierarchy degradation | `tests/e2e.rs::content_documents::custom_navigation_omits_unlinked_span_and_preserves_hierarchy` | G5-11, G5-13 |
| `E2E-SCRIPT-01` | Script/form boundary | `tests/e2e.rs::resource_resolution::script_and_form_boundary_is_explicit` | G2-24, G3-27, G8-01, G8-02, G8-03, G8-05 |
| `E2E-SCRIPT-02` | Direct SVG script rejection | `tests/e2e.rs::resource_resolution::direct_svg_script_is_safely_rejected` | G2-24, G3-05, G8-01, G8-02, G8-03, G8-05 |
| `E2E-SCRIPT-03` | Scripted fallback rejection | `tests/e2e.rs::resource_resolution::script_fallback_is_safely_rejected` | G2-24, G8-04 |
| `E2E-ACCESS-01` | Accessibility metadata/ARIA transport boundary | `tests/e2e.rs::accessibility::accessibility_metadata_and_aria_do_not_change_reading_semantics` | G8-10, G8-12, G8-14 |
| `E2E-ACCESS-02` | Audio/video playback rejection | `tests/e2e.rs::accessibility::audio_video_playback_is_explicitly_rejected` | G6-23, G6-24, G6-25, G8-09 |
| `E2E-ACCESS-03` | Generic resource byte transport | `tests/e2e.rs::accessibility::generic_resources_are_transportable_without_affecting_reading` | G6-22 |
| `E2E-ACCESS-04` | Media-overlay / direct-SMIL rejection | `tests/e2e.rs::accessibility::media_overlay_linkage_and_direct_smil_are_explicitly_rejected` | G8-06, G8-07, G8-08, G8-09 |
| `E2E-ACCESS-05` | Unlinked media does not alter reading output | `tests/e2e.rs::accessibility::unlinked_media_resources_do_not_change_reading_output` | G6-23, G6-24, G6-25, G8-07 |
| `E2E-NAV-01` | Navigation, visible TOC and reading order | `tests/e2e.rs::navigation::navigation_sources_preserve_visible_toc_and_reading_order` | G2-03, G3-14, G3-15; A-04, A-06..A-08, A-12..A-13, D-06..D-08, F-06, F-10..F-12, F-14 |
| `E2E-NAV-02` | linear=no navigation item / bodymatter boundary | `tests/e2e.rs::navigation::linear_no_navigation_item_is_retained_without_becoming_bodymatter` | G2-03; A-04, A-12, B-18, D-08, F-06, F-12 |
| `E2E-COVER-01` | Cover/thumbnail resource geometry | `tests/e2e.rs::cover_resources::cover_resource_and_library_thumbnail_contract_is_structurally_valid` | G3-18, G6-18; B-15..B-17, E-04..E-10, F-08, F-12 |
| `E2E-COVER-02` | Suppressed cover XHTML with surviving navigation children | `tests/e2e.rs::cover_resources::cover_xhtml_is_suppressed_but_cover_navigation_children_survive` | A-10, A-12 |
| `E2E-COVER-03` | Omitted cover/bodymatter landmark retargeting | `tests/e2e.rs::cover_resources::omitted_cover_bodymatter_landmarks_target_first_linear_section` | A-08, A-10, A-12, B-18, D-07, D-08, E-04..E-08, F-12 |
| `E2E-COVER-04` | Fixed-layout comic metadata and cover-resource contract | `tests/e2e.rs::cover_resources::fixed_layout_comic_metadata_and_cover_resource_contract_is_preserved` | G2-04; A-10, A-17b..A-17e, B-08..B-10, B-12..B-14, C-13, E-14 |
| `E2E-FXL-01` | Item-level pre-paginated lowering to dedicated SVG flow | `tests/e2e.rs::mixed_layout::item_level_pre_paginated_semantic_lowers_to_a_dedicated_svg_flow` | A-15, A-16, C-05..C-12, E-14, F-09 |
| `E2E-FXL-02` | Full fixed-layout page-image secondary flows | `tests/e2e.rs::fixed_layout::fixed_layout_page_images_use_secondary_svg_flows` | G6-17; A-17b..A-17e, B-08..B-10, B-12, C-05..C-11, C-13, E-14 |
| `E2E-FXL-03` | Document-local fixed viewport preservation | `tests/e2e.rs::layout_rendition_media::fixed_viewport_is_document_local_and_preserves_rawml` | G6-14 |
| `E2E-FXL-04` | Viewport grammar / duplicate declarations | `tests/e2e.rs::layout_rendition_media::viewport_grammar_and_duplicate_declarations_are_supported` | G6-14 |
| `E2E-FXL-05` | Reflowable viewport does not synthesize EXTH 126 | `tests/e2e.rs::layout_rendition_media::reflowable_viewport_does_not_emit_exth126` | G6-14 |
| `E2E-FXL-06` | MathML namespace rejection boundary | `tests/e2e.rs::layout_rendition_media::mathml_namespace_detection_rejects_only_mathml_namespace` | G3-07 |
| `E2E-FXL-07` | MathML manifest/content/fallback rejection boundary | `tests/e2e.rs::layout_rendition_media::mathml_property_content_and_fallback_are_safely_rejected` | G2-25, G3-07 |
| `E2E-RESC-01` | Publication orientation across fixed/mixed/reflowable layouts | `tests/e2e.rs::resc::publication_orientation_survives_mixed_and_reflowable_layouts` | G6-04 |
| `E2E-RESC-02` | Publication rendition and itemref overrides | `tests/e2e.rs::resc::resc_projects_publication_rendition_and_itemref_overrides` | G6-05, G6-06, G6-07, G6-10, G6-11, G6-27, G6-28, KLAY-04 |
| `E2E-RESC-03` | RESC spine semantics / SKEL topology | `tests/e2e.rs::resc::resc_projects_spine_semantics_and_skel_topology` | G6-08, G6-09, G6-26, D-17 |
| `E2E-FONT-01` | Embedded font as resolvable KF8 FONT resource | `tests/e2e.rs::embedded_fonts::checkin::embedded_font_is_preserved_as_a_resolvable_kf8_resource` | E-12 |
| `E2E-FONT-02` | IDPF font deobfuscation | `tests/e2e.rs::embedded_fonts::idpf_and_rendition::idpf_font_obfuscation_uses_unique_identifier_and_plain_font_path` | G1-07, G6-21 |
| `E2E-FONT-03` | Encryption metadata safety boundary | `tests/e2e.rs::embedded_fonts::idpf_and_rendition::encryption_invalid_shapes_and_non_font_targets_are_explicit_errors` | G1-06 |
| `E2E-RESOURCE-01` | Rendition projection and object-data resource rewrite | `tests/e2e.rs::embedded_fonts::idpf_and_rendition::rendition_projection_and_object_data_rewrite_are_observable_in_kf8` | G6-04, G6-19 |
| `E2E-UNICODE-01` | Unicode scalar/IVS/combining/supplementary preservation | `tests/e2e.rs::unicode::unicode_scalars_survive_source_xhtml_and_kf8_rawml_without_normalization` | G4-01; F-05 |
| `E2E-CSS-01` | CSS input surfaces semantic-safety scan | `tests/e2e.rs::css_safety::css_input_surfaces_are_checked_for_unsupported_semantics` | G7-01, G7-17 |
| `E2E-CSS-02` | Unsupported CSS semantics explicit rejection | `tests/e2e.rs::css_safety::unsupported_css_semantics_reject_with_feature_errors` | G7-09, G7-10, G7-25, G7-26 |
| `E2E-CSS-03` | Selector/at-rule/custom-property transport | `tests/e2e.rs::css_transport::selectors::css_transport_preserves_selectors_at_rules_and_variables` | G2-17, G4-02, G7-01 |
| `E2E-CSS-04` | Inline style local-URL projection | `tests/e2e.rs::css_transport::inline_urls` | G7-03, G7-15 |
| `E2E-CANON-01` | Canonical end-to-end semantic and KF8-structure suite | `tests/canonical_book_e2e.rs (suite)` | G2-01, G2-02, G2-04, G2-05, G3-01, G3-18, G3-21, G3-22, G3-23, G3-24, G3-25, G3-26, G4-01, G4-02, G6-01, G6-17, G6-18, G7-17, G7-19, G7-20, G7-21, G7-22, G7-23; A-01, A-03..A-08, A-12..A-14, B-01..B-07, B-18..B-22, C-01..C-03, D-06..D-10, D-12, D-14..D-15, E-01..E-03, F-01..F-08, F-10..F-14 |
| `E2E-CANON-02` | Canonical cover/TOC landmark normalization | `tests/canonical_book_e2e.rs::sovereign_stars_normalizes_cover_and_toc_landmarks_to_distinct_kf8_targets` | A-11, E-04, E-06, F-10..F-12 |
| `E2E-CANON-03` | Canonical lists/spans/writing topology | `tests/canonical_book_e2e.rs::sovereign_stars_materializes_ordered_lists_spans_and_writing_topology` | G6-17, G7-17, G7-19, G7-20, G7-21, G7-22, G7-23; C-14..C-16, B-19, B-20, F-01, F-02, F-07 |
| `E2E-CANON-04` | Canonical FCIS shape for multi-flow output | `tests/canonical_book_e2e.rs::kf8_fcis_uses_evidenced_canonical_shape_for_nine_flows` | D-11 |
| `E2E-BINARY-01` | PalmDOC byte identity / text-record geometry | `tests/large_fixture_e2e.rs::legacy_compression::palmdoc_payloads_remain_byte_identical_to_the_legacy_encoder` | B-02, B-04, B-05a; applicable Layer D text-record geometry |
| `E2E-BINARY-02` | Large RawML and index geometry | `tests/large_fixture_e2e.rs::index_geometry::crime_and_punishment_exercises_large_rawml_and_index_geometry` | G6-01; B-01..B-07, B-18..B-22, C-01..C-03, D-01..D-10, D-12, D-14..D-15, E-01..E-03, E-11, F-05..F-08, F-10..F-14 where exercised |
| `E2E-BINARY-03` | Multi-detail INDX geometry | `tests/large_fixture_e2e.rs::index_geometry::sovereign_stars_exercises_multi_detail_indx_geometry` | D-16 |

## Supporting / diagnostic evidence

The following evidence remains intentionally outside primary E2E numbering:

- `tests/characterization.rs::kindle_css::*` — Kindle CSS writer/reference comparisons.
- `tests/e2e.rs::css_transport::kindle_writer::style_attribute_projection_preserves_supported_declarations` — supporting CSS writer hardening.
- `tests/e2e.rs::css_transport::kindle_writer::comment_prefixed_declarations_are_projected_safely` — supporting CSS writer hardening.
- `tests/e2e.rs::css_transport::kindle_writer::function_parentheses_and_semicolons_preserve_following_declarations` — supporting CSS writer hardening.
- `tests/e2e.rs::css_transport::kindle_writer::active_stylesheet_graph_controls_css_validation` — supporting CSS validation hardening.
- EPUB characterization functions — diagnostic input-boundary and writer-observation support.
- `B-11` — EXTH 125 exact semantics remain diagnostic.
- `D-13` — DATP generation/payload remains diagnostic.
- `C-04` / `E-13` — conditional/non-canonical contracts until a supported input exercises them.
- Layer `L` and the supplemental ASIN cover-fetch observation — Physical Kindle device evidence.
- `api_cli_e2e.rs` — public API / CLI contract outside Layers A–F and the EPUB/KF8 semantic ownership table.


# Part VII — Release Acceptance

## Release acceptance criteria

Mandatory release acceptance requires:

```text
all mandatory A–F items exercised by an applicable fixture
AND
Amazon reader-visible semantics are accounted for
AND
every supported EPUB input that is reachable through the supported path is either
faithfully projected or explicitly rejected before output when the self writer would
otherwise lose meaning
AND
no known semantic mismatch in the canonical AozoraEpub3 path
AND
all machine-verifiable binary geometry passes
AND
applicable same-input KindleGen comparisons show no unexplained semantic difference
AND
Physical Kindle acceptance remains successful for the designated device acceptance books
```

For an Amazon-reader-visible semantic with a reachable supported EPUB source, current
self-writer loss must result in a faithful projection or an explicit temporary `SAFE
REJECT`; it must not be silently ignored. `SAFE REJECT` is a semantic-loss safety
boundary, not final support or closure of the feature.

Extended-Scope failures do not automatically block release. A known Extended-Scope
semantic mismatch may remain open when it is explicitly documented, does not
reproduce in the canonical path, and Physical Kindle acceptance remains successful.
An Extended-Scope failure does block release when investigation shows that the same
defect affects a mandatory canonical contract.

This unified audit is the authoritative audit model for current and future tests, fixes, and documentation.

# Evidence sources

Primary references used by the audit include:

- EPUB 3.3 and EPUB Reading Systems 3.3, including the Package Rendering Vocabulary and deprecated-but-defined rendering properties/values.
- Current Amazon Kindle Publishing Guidelines, including fixed-layout text-pop-up / Region Magnification and synthetic-spread guidance.
- MobileRead MOBI documentation and other KF8/MOBI technical references.
- Same-input Amazon KindleGen V2.9 build 1029-0897292 final KF8/V8 output.
- calibre, KindleUnpack, and Kindling implementation/reference evidence.
- Current production source and executable E2E evidence.
- Physical Kindle acceptance evidence where applicable.
