# KindleGen Semantic Parity Audit — A–F Model

> Audit status updated: 2026-09-06

## 1. Audit purpose

This audit verifies that EPUB source semantics required by the supported
AozoraEpub3-to-AZW3 conversion path are preserved correctly in generated
KF8/AZW3 output.

The audit distinguishes:

- EPUB source semantics,
- KF8/MOBI binary structure,
- KF8 lowering behavior,
- navigation and position geometry,
- resource geometry,
- semantic preservation.

The audit does **not** require byte-identical output with KindleGen.

KindleGen is used as a same-input writer reference where KF8/MOBI behavior
is undocumented or only reverse-engineered.

Physical Kindle is the final acceptance oracle for reader-visible failures,
but it is not treated as a binary format specification or as a CSS/SVG
renderer capability matrix.

---

# 2. Evidence classes

| Evidence class | Meaning |
|---|---|
| `SPEC` | Public normative standard, primarily EPUB / W3C specifications |
| `AMAZON-GUIDE` | Current Amazon Kindle Publishing Guidelines |
| `REFERENCE` | Reverse-engineered or implementation reference such as MobileRead MOBI Wiki, calibre, KindleUnpack, Kindling |
| `KINDLEGEN` | Same-input KindleGen writer behavior |
| `DEVICE` | Physical Kindle acceptance evidence |
| `PRODUCT-CONTRACT` | Explicit converter behavior guaranteed by this project |
| `PRODUCT-SCOPE` | Explicit supported / extended product boundary |
| `CANONICAL-INPUT` | Behavior required because it occurs in canonical AozoraEpub3 input |

## Evidence-source policy

Primary source priority:

1. Amazon Kindle Publishing Guidelines — current Web edition
2. Amazon Kindle Publishing Guidelines PDF — current full-document release
3. EPUB / W3C specifications
4. MobileRead MOBI Wiki / calibre format documentation
5. same-input KindleGen
6. calibre / KindleUnpack / Kindling implementation evidence
7. Physical Kindle acceptance evidence

Do not invent a Kindle field, EXTH record, FDST shape, DATP payload,
index field, vendor value, or serialization rule when the available
evidence does not establish it.

---

## Source-inspection status

The `Source inspection` column summarizes what is established by the current
production source and direct automated evidence. It does not replace same-input
KindleGen comparison or Physical Kindle acceptance.

- `IMPLEMENTED` — the production path and relevant invariants are present.
- `IMPLEMENTED; E2E VERIFIED` — the contract is implemented and directly exercised by E2E coverage.
- `SOURCE-INCONCLUSIVE` — source inspection alone cannot establish the full contract.
- `CONDITIONAL-GAP` — no general implementation is required until a supported input exercises the conditional path.
- `DIAGNOSTIC` — intentionally unresolved and not promoted to a parity requirement.

Qualified variants of these statuses add scope or evidence detail without changing
the underlying classification.

---

# 3. Layer A — EPUB Source Contract

| ID | Contract | Evidence basis | Requirement status | Source inspection |
|---|---|---|---|---|
| A-01 | Preserve core package metadata required by the supported conversion path | `SPEC` | REQUIRED | `IMPLEMENTED` |
| A-02 | Metadata refinements not projected into KF8 are intentionally unprojected rather than accidentally lost | `SPEC + PRODUCT-CONTRACT` | REQUIRED | `SOURCE-INCONCLUSIVE` |
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
| A-17a | Recognize and retain `rendition:orientation` input semantics without inventing a dedicated KF8 projection where no established mapping exists | `SPEC + AMAZON-GUIDE + REFERENCE` | OPTIONAL / NON-CANONICAL FOR AOZORAEPUB3 | `IMPLEMENTED INPUT RETENTION; NO KF8 SERIALIZATION REQUIREMENT` |
| A-17b | Preserve Amazon fixed-layout publication metadata | `AMAZON-GUIDE` | REQUIRED IF PRESENT | `IMPLEMENTED` |
| A-17c | Preserve `original-resolution` | `AMAZON-GUIDE` | REQUIRED FOR APPLICABLE FXL | `IMPLEMENTED` |
| A-17d | Preserve `orientation-lock` | `AMAZON-GUIDE` | REQUIRED IF PRESENT | `IMPLEMENTED` |
| A-17e | Preserve explicit canonical AozoraEpub3 `book-type=\"comic\"` when present; do not synthesize other book types heuristically | `AMAZON-GUIDE + CANONICAL-INPUT` | REQUIRED WHEN PRESENT IN CANONICAL INPUT | `IMPLEMENTED; EXPLICIT COMIC VERIFIED` |
| A-17f | Preserve `primary-writing-mode` where applicable | `AMAZON-GUIDE` | REQUIRED IF PRESENT | `IMPLEMENTED` |

---

# 4. Layer B — MOBI/KF8 Header & Metadata

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

# 5. Layer C — KF8 Content Lowering

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

# 6. Layer D — KF8 Index / Navigation / Position Geometry

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

---

# 7. Layer E — KF8 Resource Geometry

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

# 8. Layer F — EPUB → AZW3 Semantic Preservation

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

# 9. Product Scope and Known Non-Blocking Gaps

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

# 10. E2E Coverage

The A–F audit is backed by focused E2E tests plus the broad Sovereign Stars fixture.
The mappings below identify primary ownership; individual tests may exercise
additional contracts.

## `cover_e2e.rs`

Primary coverage:

```text
A-08
A-09
A-10
A-11
B-15
B-16
B-17
E-04
E-05
E-06
E-07
E-08
E-09
E-10
F-12
```

The hard cover/thumbnail assertions and the independent base32 encoder/decoder
assertion used for EXTH 129 are retained.

## `navigation_e2e.rs`

Primary coverage:

```text
A-04
A-06
A-07
A-08
A-12
D-06
D-07
D-08
F-06
F-10
F-11
F-12
```

## `sovereign_stars_e2e.rs`

Broad semantic and KF8-structure coverage across A–F. In particular, the current
fixture directly exercises:

```text
A-11 — suppressed cover landmark remains distinct from the TOC and resolves to the native cover resource
C-14 — ordered-list ordinal materialization, including the large nested list
C-15 — span AID coverage
C-16 — mixed-writing source DOM preservation
D-11 — canonical 52-byte KF8 FCIS shape on a nine-flow FDST artifact
```

It does not claim F-09 unless the required pre-paginated page-flow topology is
independently verified.

## `unicode_e2e.rs`

Primary coverage: `F-05`. Required preservation cases include IVS, combining
dakuten, combining handakuten, and supplementary-plane characters.
Normalization-based comparison is not permitted.

## `palmdoc_e2e.rs`

Primary coverage: `B-02`, `B-05a`, plus applicable text-record geometry from Layer D.

## `large_index_e2e.rs`

Primary coverage:

```text
D-01
D-02
D-03
D-04
D-05
D-15
D-16 when the multi-detail threshold is crossed
```

The canonical Sovereign Stars fixture closes D-16 with a FRAG INDX containing
`entry_count=2887`, `detail_count=2`, and detail ranges `[2769, 118]`. The shared
INDX parser verifies routing rows, detail IDXT ranges, count reconstruction, and
ordered/in-range control-record pointers.

## `mixed_layout_fixed_page_flow_e2e.rs`

This test covers item-level pre-paginated lowering rather than asserting a general
Amazon-supported mixed-layout publication mode.

Primary coverage:

```text
A-15
A-16
C-05
C-06
C-07
C-08
C-09
C-10
C-11
C-12
E-14
F-09
```

It verifies that a canonical pre-paginated spine page is emitted as an independent
secondary flow, represented in FDST, referenced from main RawML through the expected
`kindle:flow:` URI, and kept distinct from surrounding reflowable content.

## `fixed_layout_flow_e2e.rs`

Extended-Scope full fixed-layout coverage:

```text
A-17*
B-08
B-09
B-10
B-12
C-05
C-06
C-07
C-08
C-09
C-10
C-11
C-13
E-14
```

The fixture also verifies the evidenced cover-page reading-order rule for an explicit
publication-level pre-paginated comic. B-11, B-13, and B-14 are not forced to
KindleGen numeric parity without additional evidence.

## `e12_embedded_font_e2e.rs`

Primary coverage: `E-12`. The source EPUB contains a packaged TTF referenced by
`@font-face`; the generated AZW3 contains a non-empty FONT record, preserves the
font bytes, and rewrites the CSS reference to an in-range `kindle:embed` resource.

## `public_api_and_cli.rs`

This is a separate public API / CLI contract test outside Layers A–F.

---

# 11. Physical Kindle Acceptance

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
`Sovereign Stars` Physical Kindle diagnostic run. It does not claim 13 dedicated
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

---

# 12. Release Acceptance Criteria

Mandatory release acceptance requires:

```text
all mandatory A–F items exercised by an applicable fixture
AND
no known semantic mismatch in the canonical AozoraEpub3 path
AND
all machine-verifiable binary geometry passes
AND
applicable same-input KindleGen comparisons show no unexplained semantic difference
AND
Physical Kindle acceptance remains successful for the designated device acceptance books
```

Extended-Scope failures do not automatically block release. A known Extended-Scope
semantic mismatch may remain open when it is explicitly documented, does not
reproduce in the canonical path, and Physical Kindle acceptance remains successful.
An Extended-Scope failure does block release when investigation shows that the same
defect affects a mandatory canonical contract.

The A–F model is the authoritative public audit model for current and future tests,
fixes, and documentation.
