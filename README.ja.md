# AozoraEpub3-to-azw3

[English](README.md) | [日本語](README.ja.md)

EPUBをKindle向けのKF8-only AZW3へ変換するRustライブラリおよびCLIです。

主なサポート対象は、[AozoraEpub3](https://github.com/kyukyunyorituryo/AozoraEpub3) が生成したEPUBです。

日本語の縦書き、右から左へのページ進行、ルビ、傍点、ナビゲーション、Kindle向けCSS変換を重視しています。

あわせて、[**EPUB 3.3**](https://www.w3.org/TR/epub-33/) **の仕様を広範囲にカバー**しており、KF8/AZW3への変換について広範な監査と検証を行っています。

任意のEPUBジェネレーターとの互換性を保証するものではありません。

[AozoraEpub3_Lite](https://github.com/Rumia-Channel/AozoraEpub3_Lite) への対応も予定しています。

## ダウンロード

最新版は[Releases](https://github.com/cbz-tools/AozoraEpub3-to-azw3/releases/latest)からダウンロードできます。

| プラットフォーム | パッケージ |
|---|---|
| Windows x64 | `aozoraepub3-to-azw3-vX.Y.Z-windows-x64.zip` |
| Linux x64 | `aozoraepub3-to-azw3-vX.Y.Z-linux-x64.tar.gz` |
| macOS Apple Silicon | `aozoraepub3-to-azw3-vX.Y.Z-macos-arm64.tar.gz` |

アーカイブを展開し、`AozoraEpub3-to-azw3` を直接実行してください。

### Cargoでインストール

Rust 1.85以降が必要です。

```bash
cargo install aozoraepub3-to-azw3
```

## クイックスタート

EPUBを変換します。

```bash
AozoraEpub3-to-azw3 input.epub
```

出力先を省略した場合、入力パスの拡張子を `.azw3` に置き換えたパスが使用されます。

## 主な機能

- KF8-only AZW3出力
- 日本語縦書き
- 右から左へのページ進行
- ルビ・傍点
- Kindle向けナビゲーション・読書順序
- 埋め込みフォント
- Kindle向けCSS変換
- PalmDOC圧縮
- カバーリソース・書籍内カバー表示
- EPUB 3.3仕様の広範な対応
- CLIに加えてRustライブラリAPIを提供


## スクリーンショット

以下は、同じ **Sovereign Stars Vol.1** のテスト書籍をKindle実機で表示した比較です。

### AozoraEpub3-to-azw3（AZW3）

| 表紙 | 目次 | 縦書き本文 |
|---|---|---|
| [![表紙](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/AozoraEpub3-to-azw3-01.JPEG)](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/AozoraEpub3-to-azw3-01.JPEG) | [![目次](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/AozoraEpub3-to-azw3-02.JPEG)](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/AozoraEpub3-to-azw3-02.JPEG) | [![縦書き本文](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/AozoraEpub3-to-azw3-03.JPEG)](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/AozoraEpub3-to-azw3-03.JPEG) |

### KindleGen（MOBI）

| 表紙 | 目次 | 縦書き本文 |
|---|---|---|
| [![表紙](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/kindlegen-01.JPEG)](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/kindlegen-01.JPEG) | [![目次](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/kindlegen-02.JPEG)](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/kindlegen-02.JPEG) | [![縦書き本文](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/kindlegen-03.JPEG)](https://raw.githubusercontent.com/cbz-tools/AozoraEpub3-to-azw3/main/docs/assets/kindlegen-03.JPEG) |

この比較では、読者から見える差は確認できませんでした。

## 目的

KindleGenは長年Amazonから入手できない状態が続いており、新しいツールから利用する依存先としては現実的ではなくなっています。

このプロジェクトは、EPUBをKF8-only AZW3へ直接変換する、継続的にメンテナンスされるRustライブラリおよびCLIを提供します。

従来のMOBI7/KF8二重形式ファイルは意図的に生成しません。Kindle Touch (4th Generation) より前のKindle端末はサポート対象外です。現在の対象ではKF8-only出力とすることで実装を小さく保ち、不要になった旧形式の複雑さを持ち込まずに済みます。

## KindleGenとの比較

| | KindleGen | aozoraepub3-to-azw3 |
|---|---|---|
| 入力 | EPUB、HTML/OPF、その他の対応入力 | EPUB |
| 出力 | 従来形式 + KF8 Kindle形式 | KF8-only AZW3 |
| 日本語縦書き / ルビ | 対応 | 対応、Kindle実機で検証済み |
| 書籍ビューアー内のカバー表示 | 対応 | 対応、Kindle実機で検証済み |
| Kindleライブラリのカバーサムネイル | 対応 | 未対応 |
| [Kindle Touch (4th Generation) より前の端末](https://digprjsurvey.amazon.com/csad/help/node/GK33S847NN4V6Y83) | サポート対象 | サポート対象外 |
| 入手性 | Amazonからの配布終了 | オープンソースで継続的にメンテナンス |

## パフォーマンス

以下は、実データを用いた測定例の一つです。

入力準備：

| 段階 | サイズ |
|---|---:|
| 元TXT（テキストのみ、画像なし） | 49.58 MiB (51,992,309 bytes) |
| AozoraEpub3で生成したEPUB | 17.92 MiB (18,789,218 bytes) |

両コンバーターには、同じ生成済みEPUBを入力しました。

| | KindleGen | aozoraepub3-to-azw3 |
|---|---:|---:|
| 変換時間 | 122.714 s | 4.869 s |
| 出力サイズ | 83.70 MiB (87,761,803 bytes) | 47.42 MiB (49,723,912 bytes) |

各コンバーターを同じEPUBに対して5回実行し、最速の1回と最遅の1回だけを除外した残り3回の算術平均を変換時間として記載しています。この測定では、`aozoraepub3-to-azw3` は約25.2倍高速に変換を完了し、生成されたAZW3はKindleGenのMOBIより43.34%小さくなりました。

`aozoraepub3-to-azw3` はデフォルトのPalmDOC圧縮で実行しています。

結果は入力、ツールのバージョン、ハードウェアによって異なります。TXTからEPUBを生成する時間は変換時間に含めていません。

## CLI

```text
AozoraEpub3-to-azw3 <input.epub> [-o <output.azw3>] [-c0 | -c1] [-verbose]
    [-dont_append_source] [-donotaddsource]
```

`-c1` がデフォルトで、PalmDOC圧縮を使用します。`-c0` はテキストレコードを非圧縮で格納します。`-dont_append_source` と `-donotaddsource` はKindleGen互換の無処理オプションとして受け付けます。このコンバーターは元のEPUBを埋め込みません。`-c2`（HUFF/CDIC）および無関係なKindleGenオプションは意図的にサポートしていません。`-o` を省略すると、入力ファイルの拡張子を `.azw3` に置き換えます。

## ライブラリ

```rust
use aozoraepub3_to_azw3::{convert_bytes, Compression, ConvertOptions};

let azw3 = convert_bytes(
    &epub_bytes,
    &ConvertOptions { compression: Compression::PalmDoc },
)?;
```

このcrateは `convert_file(input, output, options)` も提供します。

`convert_file`では、変換結果はシリアライズ時に出力先ファイルへ書き込まれます。グローバルな可変変換状態、共有一時ファイル名、カレントディレクトリの変更、内部並列ランタイムは使用しません。独立した変換は安全に並行実行できます。通常のファイル書き込みと同様、完全に同じ出力パスへ複数の変換を同時実行する場合の調整は呼び出し側の責任です。

## 制限事項

Kindle Touch (4th Generation) より前のKindle端末はサポートしていません。このコンバーターは、それ以降のKindle端末向けのKF8-only AZW3を生成します。

Kindleライブラリのカバーサムネイルには現在対応していません。KF8/AZW3では、書籍内のカバー画像とKindleライブラリで使用されるサムネイルは別に扱われます。

## 検証

AozoraEpub3が生成したEPUBを、このプロジェクトの主要な検証対象としています。

その要件に加えて、**EPUB 3.3の仕様を広範囲に実装・監査**しており、KF8/AZW3への変換について幅広いEPUBの挙動を検証しています。

EPUBについては、[EPUB 3.3仕様](https://www.w3.org/TR/epub-33/)を基準にしています。

Kindle固有の変換および互換性については、[Amazon Kindle Publishing Guidelines](https://kindlegen.s3.amazonaws.com/AmazonKindlePublishingGuidelines.pdf)、同一入力に対するKindleGen出力、およびKindle実機を参照・検証しています。

読書順序、ナビゲーション、テキストの忠実性、日本語縦書き、ルビなど、読者から見える挙動についてもKindle実機で確認しています。

対応範囲、検証方法、互換性の境界、E2Eおよび実機での検証結果の詳細については、[EPUB to KF8/AZW3 Conversion Audit](docs/EPUB-to-KF8-Conversion-Audit.md)を参照してください。

この監査は、本コンバーターで検証された動作についての正式な技術記録です。

## 謝辞

このプロジェクトのKF8/MOBI形式の解析および実装では、以下のオープンソースプロジェクトおよび技術資料から多大な恩恵を受けました。

- [MobileRead Wiki — MOBI](https://wiki.mobileread.com/wiki/MOBI) — PalmDOC、MOBI/EXTHヘッダー、インデックス、レコード構造などを理解するうえで重要な資料となりました。
- [Kindling](https://github.com/ciscoriordan/kindling) — Rust製Kindleツールキットであり、KF8/MOBI構造を理解するうえで有用な参考実装となりました。
- [calibre](https://github.com/kovidgoyal/calibre) — Kindle/MOBI実装は、フォーマットの挙動や実装詳細を理解するうえで非常に有用な参考資料となりました。
- [KindleUnpack](https://github.com/kevinhendricks/KindleUnpack) — KindleGenが生成したファイルの調査・解析に広く使用しました。

これらの成果と情報を公開してくださった作者、コントリビューター、およびコミュニティの皆様に感謝します。

このプロジェクトは独立した実装であり、Amazon、MobileRead、Kindling、calibre、KindleUnpackとは関係ありません。

## ライセンス

MIT Licenseの下でライセンスされています。

詳細は[LICENSE](LICENSE)を参照してください。

サードパーティ依存関係の概要は[THIRDPARTY_LICENSES.md](THIRDPARTY_LICENSES.md)を参照してください。

## 変更履歴

リリース履歴は[CHANGELOG.md](CHANGELOG.md)を参照してください。
