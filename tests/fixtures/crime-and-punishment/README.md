# 『罪と罰』 regression fixture

## Provenance

- Title: 罪と罰
- Author: フョードル・ドストエフスキー
- Translator: 米川正夫
- Aozora Bunko No. 56656
- Original ZIP name: `56656_ruby_74439.zip`
- Original card URL: https://www.aozora.gr.jp/cards/000363/card56656.html
- Original ZIP URL: https://www.aozora.gr.jp/cards/000363/files/56656_ruby_74439.zip
- Retrieval date: 2026-09-04
- Original encoding: Shift_JIS / JIS X0208
- Source status: public-domain text distributed by Aozora Bunko

The source transform was `Shift_JIS / JIS X0208` -> BOM-free UTF-8 -> canonical
AozoraEpub3 1.1.1b33Q -> `source.epub`. No textual or semantic edits
were made. Intermediates are not retained; this directory intentionally keeps
only this README and the generated EPUB.

Normal `cargo test` consumes only `source.epub`; it does not require
the original ZIP, a generator, KindleGen, a network connection, or a device.
The purpose is large KF8 / FRAG / SKEL / INDX / PositionMap / TBS stress,
including large record-boundary and index-routing geometry.
