# AozoraEpub3-to-azw3

[English](README.md) | [日本語](README.ja.md)

[AozoraEpub3](https://github.com/kyukyunyorituryo/AozoraEpub3) が生成したEPUBを、
KF8-only AZW3へ変換するRustライブラリおよびCLIです。

[AozoraEpub3_Lite](https://github.com/Rumia-Channel/AozoraEpub3_Lite) への対応も予定しています。

日本語の縦書き、右から左へのページ進行、ルビ、傍点、Kindle向けCSS projectionを重視しています。

汎用EPUBコンバーターではなく、KindleGenの完全な代替を目的としたものでもありません。


## ダウンロード

最新版は[Releases](https://github.com/cbz-tools/AozoraEpub3-to-azw3/releases/latest)からダウンロードできます。

| プラットフォーム | パッケージ |
|---|---|
| Windows x64 | `aozora-epub3-to-azw3-v0.1.0-windows-x64.zip` |
| Linux x64 | `aozora-epub3-to-azw3-v0.1.0-linux-x64.tar.gz` |
| macOS Apple Silicon | `aozora-epub3-to-azw3-v0.1.0-macos-arm64.tar.gz` |

アーカイブを展開し、`AozoraEpub3-to-azw3` を直接実行してください。

### Cargoでインストール

Rust 1.85以降が必要です。

```bash
cargo install aozora_epub3_to_azw3
```

## スクリーンショット

以下は、同じ **Sovereign Stars Vol.1** のテスト書籍を
Kindle実機で表示した比較です。

### AozoraEpub3-to-azw3（AZW3）

| 表紙 | 目次 | 縦書き本文 |
|---|---|---|
| [![表紙](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/AozoraEpub3-to-azw3-01.JPEG)](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/AozoraEpub3-to-azw3-01.JPEG) | [![目次](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/AozoraEpub3-to-azw3-02.JPEG)](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/AozoraEpub3-to-azw3-02.JPEG) | [![縦書き本文](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/AozoraEpub3-to-azw3-03.JPEG)](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/AozoraEpub3-to-azw3-03.JPEG) |

### KindleGen（MOBI）

| 表紙 | 目次 | 縦書き本文 |
|---|---|---|
| [![表紙](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/kindlegen-01.JPEG)](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/kindlegen-01.JPEG) | [![目次](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/kindlegen-02.JPEG)](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/kindlegen-02.JPEG) | [![縦書き本文](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/kindlegen-03.JPEG)](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/kindlegen-03.JPEG) |

この比較では、読者から見える差は確認できませんでした。

## クイックスタート

EPUBを変換します。

```bash
AozoraEpub3-to-azw3 input.epub
```

出力先を省略した場合、入力パスの拡張子を `.azw3` に置き換えたパスが使用されます。

## 目的

KindleGenは長年Amazonから入手できない状態が続いており、新しいツールから利用する依存先としては現実的ではなくなっています。

このプロジェクトは、EPUBをKF8-only AZW3へ直接変換する、メンテナンスされたRustライブラリおよびCLIを提供します。

legacy MOBI7/KF8 dual-formatファイルは意図的に生成しません。Kindle Touch (4th Generation) より前のKindle端末はサポート対象外です。現在の対象ではKF8-only出力とすることで実装を小さく保ち、不要なlegacy formatの複雑さを持ち込まずに済みます。

CLIだけでなくRustライブラリでもあるため、外部のKindleGen実行ファイルを起動せず、変換をプロセス内で直接実行できます。

## KindleGenとの比較

| | KindleGen | aozora-epub3-to-azw3 |
|---|---|---|
| 入力 | EPUB、HTML/OPF、その他の対応入力 | 対応EPUB |
| 出力 | Legacy + KF8 Kindle format | KF8-only AZW3 |
| 日本語縦書き / ルビ | 対応 | 対応、Physical Kindleで検証済み |
| 書籍ビューアー内のカバー表示 | 対応 | 対応、Physical Kindleで検証済み |
| Kindleライブラリのカバーサムネイル | 対応 | 未対応 |
| [Kindle Touch (4th Generation) より前の端末](https://digprjsurvey.amazon.com/csad/help/node/GK33S847NN4V6Y83) | サポート対象 | サポート対象外 |
| 入手性 | Amazonからの配布終了 | オープンソースで継続的にメンテナンス |

## パフォーマンス

以下は、実データを用いた測定例の一つです。

入力準備：

| 段階 | サイズ |
|---|---:|
| Source TXT（テキストのみ、画像なし） | 49.58 MiB (51,992,309 bytes) |
| AozoraEpub3-generated EPUB | 17.92 MiB (18,789,211 bytes) |

両コンバーターには、同じ生成済みEPUBを入力しました。

| | KindleGen | aozora-epub3-to-azw3 |
|---|---:|---:|
| 変換時間 | 115.928 s | 4.698 s |
| 出力サイズ | 83.70 MiB (87,761,779 bytes) | 47.33 MiB (49,625,944 bytes) |

各コンバーターを同じEPUBに対して5回実行し、最速の1回と最遅の1回だけを除外した残り3回の算術平均を変換時間として記載しています。この測定では、`aozora-epub3-to-azw3` は約24.7倍高速に変換を完了し、生成されたAZW3はKindleGenのMOBIより43.45%小さくなりました。

`aozora-epub3-to-azw3` はデフォルトのPalmDOC圧縮で実行しています。

結果は入力、ツールのバージョン、ハードウェアによって異なります。TXTからEPUBを生成する時間は変換時間に含めていません。

## 対応EPUBジェネレーター

- [AozoraEpub3](https://github.com/kyukyunyorituryo/AozoraEpub3) — 対応
- [AozoraEpub3_Lite](https://github.com/Rumia-Channel/AozoraEpub3_Lite) — 対応予定

その他のEPUBジェネレーターはサポート対象外です。

## CLI

```text
AozoraEpub3-to-azw3 <input.epub> [-o <output.azw3>] [-c0 | -c1] [-verbose]
    [-dont_append_source] [-donotaddsource]
```

`-c1` がデフォルトで、PalmDOC圧縮を使用します。`-c0` はテキストレコードを非圧縮で格納します。`-dont_append_source` と `-donotaddsource` はKindleGen互換のno-opとして受け付けます。このコンバーターは元のEPUBを埋め込みません。`-c2`（HUFF/CDIC）および無関係なKindleGenオプションは意図的にサポートしていません。`-o` を省略すると、入力ファイルの拡張子を `.azw3` に置き換えます。

## ライブラリ

```rust
use aozora_epub3_to_azw3::{convert_bytes, Compression, ConvertOptions};

let azw3 = convert_bytes(
    &epub_bytes,
    &ConvertOptions { compression: Compression::PalmDoc },
)?;
```

このcrateは `convert_file(input, output, options)` も提供します。

`convert_file`では、変換結果はシリアライズ時に出力先ファイルへ書き込まれます。global mutable conversion state、共有temporary name、current-directory変更、内部parallel runtimeは使用しません。独立した変換は安全に並行実行できます。通常のファイル書き込みと同様、完全に同じ出力パスへ複数の変換を同時実行する場合の調整は呼び出し側の責任です。

## 制限事項

Kindle Touch (4th Generation) より前のKindle端末はサポートしていません。このコンバーターは、それ以降のKindle端末向けのKF8-only AZW3を生成します。

書籍ビューアー内のカバー表示には対応しており、Physical Kindleで検証済みです。

Kindleライブラリのカバーサムネイルには現在対応していません。KF8/AZW3では、書籍内のカバー画像とKindleライブラリで使用されるサムネイルは別に扱われます。

## 検証

AozoraEpub3 EPUBの変換出力はKindleGen出力との比較監査およびPhysical Kindleでの確認を実施しており、EPUB semantic ingestion、KF8/AZW3 serialization、end-to-endのreading order、navigation、text fidelityを対象としています。

現在のA–F監査では、必須contractの実装・検証を完了しており、canonical AozoraEpub3 pathに既知のsemantic mismatchはありません。

また、reader-visible semantic contract 13項目すべてを確認しており、Physical Kindle Device Evidenceは13 / 13 `CONFIRMED PASS` です。

完全な結果、方法、分類基準、evidenceについては、[KindleGen semantic parity audit](docs/KindleGen-Semantic-Parity-Audit.md)を参照してください。

## 謝辞

このプロジェクトのKF8/MOBI format解析および実装では、以下のオープンソースプロジェクトおよびtechnical referenceから多大な恩恵を受けました。

- [MobileRead Wiki — MOBI](https://wiki.mobileread.com/wiki/MOBI) — PalmDOC、MOBI/EXTH header、index、record構造などを理解するうえで重要なformat referenceとなりました。
- [Kindling](https://github.com/ciscoriordan/kindling) — Rust製Kindle toolkitであり、KF8/MOBI構造を理解するうえで有用なreferenceとなりました。
- [calibre](https://github.com/kovidgoyal/calibre) — Kindle/MOBI実装は、format behaviorおよびimplementation detailsを理解するうえで非常に有用なreferenceとなりました。
- [KindleUnpack](https://github.com/kevinhendricks/KindleUnpack) — KindleGen生成ファイルのinspectionおよびanalysisに広く使用しました。

これらの成果と情報を公開してくださった作者、contributor、およびcommunityの皆様に感謝します。

このプロジェクトは独立した実装であり、Amazon、MobileRead、Kindling、calibre、KindleUnpackとは関係ありません。

## ライセンス

MIT Licenseの下でライセンスされています。

詳細は[LICENSE](LICENSE)を参照してください。

third-party dependencyの概要は[THIRDPARTY_LICENSES.md](THIRDPARTY_LICENSES.md)を参照してください。

## 変更履歴

リリース履歴は[CHANGELOG.md](CHANGELOG.md)を参照してください。
