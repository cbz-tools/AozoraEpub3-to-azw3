# Automatic A–F audit coverage

The authoritative requirements are in the
[KindleGen-Semantic-Parity-Audit.md](../docs/KindleGen-Semantic-Parity-Audit.md).
The executable mapping is maintained by the test names and comments in the test
files below. These tests verify machine-observable EPUB/KF8 semantics and do not
claim to replace Physical Kindle acceptance.

## Fixture ownership

Checked-in fixtures are repository-owned inputs under
`fixtures/sovereign-stars/` and `fixtures/crime-and-punishment/source.epub`.
Navigation, cover, Unicode, and fixed-page topology tests use independent
in-memory EPUB recipes from `tests/support`.

The `tests/fixtures/public-api-and-cli/source.txt` file is the provenance and
regeneration source for the public API / CLI fixture. The checked-in E2E input
is `tests/fixtures/public-api-and-cli/source.epub`; normal E2E and CI runs use
that EPUB directly and do not regenerate it via AozoraEpub3.

| Test | A–F coverage | Input fixture |
| --- | --- | --- |
| `public_api_and_cli.rs` | Public API / CLI contract outside the A–F parity rows | `fixtures/public-api-and-cli/source.epub` |
| `sovereign_stars_e2e.rs` | A-01, A-03..A-14 (including direct A-11 cover/TOC landmark checks); C-14, C-15, C-16; direct D-11 FCIS canonical-shape checks; applicable B/C/D/E; F-01..F-08 and F-10..F-14 | `fixtures/sovereign-stars/` |
| `navigation_e2e.rs` | A-04, A-06..A-08, A-12; D-06..D-08; F-06, F-10..F-12 | in-memory navigation recipes |
| `cover_e2e.rs` | A-08..A-11; B-15..B-17; E-04..E-10; F-12 | in-memory cover recipes |
| `unicode_e2e.rs` | F-05, including IVS and combining/supplementary scalars | in-memory Unicode recipe |
| `palmdoc_e2e.rs` | B-02, B-05a and text-record geometry | self-created test data |
| `large_index_e2e.rs` | B core, C-01..C-03, D-01..D-16, E-01..E-11, applicable F rows | `fixtures/crime-and-punishment/source.epub`; D-16 uses `fixtures/sovereign-stars/` |
| `e12_embedded_font_e2e.rs` | E-12 FONT-specific source manifest/CSS and generated KF8 resource addressing; reads the checked-in EPUB directly | `fixtures/embedded-font/source.epub` |
| `mixed_layout_fixed_page_flow_e2e.rs` | A-15, A-16, C-05..C-12, E-14, F-09 | independent mixed-layout micro-fixture |
| `fixed_layout_flow_e2e.rs` | A-17*, B-08..B-10, B-12..B-14, C-05..C-13, E-14 | independent EXTENDED-SCOPE fixture |

The Sovereign Stars C-layer additions are owned by the same E2E:

- C-14 — explicit ordered-list ordinal materialization, including nested reset and outer continuation
- C-15 — span AID coverage
- C-16 — mixed-writing-mode source DOM preservation and absence of a synthetic wrapper
- A-11 — source cover.xhtml and nav.xhtml landmark targets lower to distinct native-cover embed and nav-position references
- D-11 — nine-flow Sovereign Stars output directly asserts the canonical 52-byte FCIS shape, field @12, text-length equality, and pointer validity

## Fixed-page contracts

`mixed_layout_fixed_page_flow_e2e.rs` is the mandatory regression for
item-level pre-paginated source semantic preservation. It verifies that the
fixed page is recognized, lowered to a dedicated SVG secondary flow, referenced
from main RawML, represented in FDST, and kept between the two reflowable
sections. Its expected topology is built from fixture semantics, not from the
production flow encoder.

`fixed_layout_flow_e2e.rs` is an extended diagnostic for a small full
fixed-layout publication. It verifies applicable publication metadata and page
flow contracts, but does not require guessed EXTH 125 or DATP parity.

C-16 primary ownership is `sovereign_stars_e2e.rs`; the mixed-layout regression
may provide optional topology coverage while retaining its existing C-05..C-12
and F-09 responsibilities.

## Coverage boundaries

- `D-16` is `IMPLEMENTED; E2E VERIFIED`: the canonical Sovereign Stars
  fixture observes a FRAG INDX with `entry_count=2887`, `detail_count=2`,
  and detail ranges `[2769, 118]`.
- `E-12` is `IMPLEMENTED; E2E VERIFIED` for the embedded-FONT path only;
  `E-13` remains conditional for generic non-image resources beyond FONT.
- `B-11` (EXTH 125) and `D-13` (DATP) remain diagnostic/unresolved; numeric
  KindleGen imitation is prohibited.
- EXTH 127/128 are required to be absent when the source has no corresponding
  zero-gutter/zero-margin semantic.
- Full image-only fixed-layout acceptance and Physical Kindle behavior remain
  separate evidence axes.
- No test is a CSS/SVG renderer-capability or pixel-fidelity matrix.
