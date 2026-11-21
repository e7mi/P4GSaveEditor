# P4G Save Editor

Persona 4 Golden (Steam) のセーブデータを編集する Web アプリです。編集ロジックは Rust (WebAssembly)、UI はブラウザで動作します。セーブファイルはすべてブラウザ内で処理され、外部へ送信されません。

旧 Windows GUI 版は `rust-gui` ブランチにあります。

## 使い方

デプロイされたページを開くか、ローカルで `web/` フォルダを静的ホストしてブラウザで開きます (例: `python -m http.server`)。

1. ゲームを終了します。
2. `フォルダを開く` で Steam のセーブフォルダを選ぶか、`ファイルを開く` で `dataXXXX.bin` を選びます (ドラッグ&ドロップも可)。
3. 上部のスロット表示で編集するセーブを選びます。
4. 左のメニューから項目を選び、値を変更します。
5. `保存` を押します。ブラウザが File System Access API に対応していればファイルへ直接書き戻します。未対応の場合は編集済みの `.bin` / `.binslot` がダウンロードされるので、元のフォルダに上書きしてください。
6. ゲームを起動してセーブを読み込みます。

セーブデータの場所は通常:

```text
C:\Program Files (x86)\Steam\userdata\<SteamID>\1113000\remote
```

`dataXXXX.bin` と `dataXXXX.binslot` の組が 1 スロットです。`.binslot` がない場合は新しく生成されます。

直接書き戻しが使えるのは Chrome / Edge などの Chromium 系ブラウザです。Firefox / Safari ではダウンロード方式になります。

## 編集できる項目

| ページ | 内容 |
| --- | --- |
| 基本 | 名前、所持金、レベル、累計XP、HP/SP、プレイ時間、難易度、社交ステータス、現在のパーティ、実績カウンタ |
| 日付 | 現在の日付と時間帯、次に進む日付と時間帯 |
| 仲間 | メンバーのHP/SP、状態異常の解除、ペルソナ、装備の変更 |
| ペルソナ | 主人公の所持枠12個のレベル、ステータス、スキル |
| 全書 | 249枠の登録、初期化、解除、未登録のみの表示 |
| コミュ | 23枠のランクと好感度 |
| アイテム | 2559枠の所持数、全アイテムの一括追加と全削除、カテゴリと検索による絞り込み |
| ツール | チェックサムの再確認、セッション内バックアップと復元、別スロットへの複製、JSON書き出し |

保存時にはヘッダチェックサム、セーブMD5、スロットMD5を再計算します。

## 注意

- セーブの編集は自己責任で行ってください。大切なセーブは事前にコピーしておくことを勧めます。セッション内バックアップはブラウザを閉じると消えます。
- ゲームを起動したまま保存すると、ゲーム終了時に上書きされることがあります。必ずゲームを終了してから編集してください。
- Steam Cloudを使っている場合、クラウド側のデータで上書きされることがあります。同期の競合が出たらSteamの状態を確認してください。
- 名前はセーブ内のShift-JIS領域とPC用のUTF-8領域の両方に書き込みます。ゲーム内の表示が崩れた場合は、ゲーム側で一度保存し直してください。
- レベルと累計XPは独立した項目です。レベルだけを変えると、次のレベルまでの必要XPが合わないことがあります。
- アイテム、ペルソナ、スキルはゲーム内の日本語名で表示します。日本語名が取得できない未使用枠は`未使用`と表示されます。`詳細`をオンにすると、未使用のアイテムやペルソナも一覧に表示されます。
- 最大HP/SPはゲーム側の計算に依存します。上限を超える値を入れた場合、ゲーム内で補正されることがあります。

## 開発

Rust 1.85 以降と wasm32 ターゲット、wasm-bindgen-cli が必要です。

```powershell
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --locked
```

wasm と JS バインディングの生成:

```powershell
cargo build -p p4g-wasm --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir web/pkg target/wasm32-unknown-unknown/release/p4g_wasm.wasm
```

`web/pkg` は生成物なのでコミットしません。`web/` 全体を静的ホストすれば動きます。

テスト (実セーブがある環境では実ファイルで検証します):

```powershell
cargo test -p p4g-core
node tests/wasm-smoke.mjs
```

構成:

- `crates/p4g-core`: セーブ解析・書き換えロジック (`save.rs`) とゲームデータ (`game.rs` + TSV)
- `crates/p4g-wasm`: wasm-bindgen ラッパー
- `web`: フロントエンド (素の HTML/CSS/JS、ビルド不要)

GitHub Actions の `build` ワークフローが fmt / clippy / test / wasm ビルドを実行し、`deploy` ワークフローが `web/` を GitHub Pages にデプロイします (リポジトリ設定で Pages のソースを GitHub Actions にしてください)。

## データ出典

- fendevel/P4G-Save-Tool: セーブ構造とデータベース
- vware/P4G-Save-Tool: 構造の解析
- zarroboogs/p4g-ct: アイテム、ペルソナ、スキル、ソーシャルリンクのデータ
- zarroboogs/p4g-saveconv: `.binslot`の形式とMD5の計算方法
- cfxxldev/P4GPC_Savegame_Helper: チェックサムの更新方法
- Amicitia Wiki / tge-was-taken/Atlus-Script-Tools: 日本語名テーブルの構造と文字コード
- gameludens.com: ペルソナとスキルの日英対応

TSVデータは上記プロジェクトの内容を元にしています。データは各出典の条件に従います。

## ライセンス

ソースコードは MIT License または Apache License 2.0 のいずれかで利用できます。全文は `LICENSE-MIT` と `LICENSE-APACHE` にあります。

Persona 4 Golden に由来する名称やアイコンなどの素材は Atlus / SEGA に帰属し、これらのライセンスの対象外です。詳細は `NOTICE` を参照してください。
