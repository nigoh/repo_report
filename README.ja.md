# repo_report

`repo-report` は単一ファイルの Bash CLI です。ディレクトリツリーを走査し、ネストされたすべての git リポジトリ（`.git` ディレクトリおよび `.git` gitfile ポインタ — Google の `repo` ツールやサブモジュールで使用される形式）を検出してそのステータスを表示します。2 つのモードがあります：

- **インタラクティブ TUI**（実端末上でのデフォルト）— アニメーション付きの `🔴 LIVE · REPO REPORTER` ニュースティッカーが上部をスクロールしながら、ワーカーがスクロール・フィルタ・ソート可能なリストに結果を流し込みます。`?` キーで全キーバインドを確認できます。
- **非インタラクティブ**（パイプ時、または `--format` / `-n` 指定時）— `table` / `tsv` / `json` 形式のレポートを並列出力します。パイプライン、CI、および `/repo-report` Claude Code スキルに適しています。

通常のツール（`gita`、`mr`、`ghq`、Google `repo`）はリポジトリの事前登録が必要だったり、`.repo` ワークスペース全体で「すべて最新か？」を確認するためのコンパクトな機械可読レポートを出力できなかったりするため、このツールを作成しました。

## インストール不要で今すぐ使う

クローン直後からスクリプトをそのまま実行できます — インストール不要：

```sh
git clone https://github.com/nigoh/repo_report.git
cd repo_report
./bin/repo-report /path/to/workspace
```

## インストール（任意 — `repo-report` を PATH に追加する場合）

```sh
# Makefile 経由（デフォルトは /usr/local/bin）
make install
# カスタムプレフィックスを指定する場合
make install PREFIX=~/.local

# 手動でコピー
install -m0755 bin/repo-report /usr/local/bin/repo-report
# またはシンボリックリンク
ln -s "$PWD/bin/repo-report" ~/.local/bin/repo-report
```

アンインストール：`make uninstall`

依存関係：`bash`（>=4）、`git`、`find`、`xargs`、`awk`、`mkfifo`。  
`column` は任意（`--format table` でのテーブル整形に使用）。

## インタラクティブモード

引数なし（またはパス指定）で実端末上で実行します：

```sh
repo-report /path/to/workspace
```

レイアウト：

```
╭──────────────────────────────────────────────────────────────────╮
│ 🔴 LIVE · REPO REPORTER · scanned 42/120 · ⚡ 3 BEHIND · ⚠ 1 DIRTY │  ← スクロールするティッカー
├──────────────────────────────────────────────────────────────────┤
│ root:.  jobs:8  fetch:off  sort:path  scanned:42/120  behind:3   │  ← ステータスバー
├▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓────────────────────────────────────┤  ← スキャン進捗バー
│ > workspace/proj-a        main  0a1b2c3  up-to-date  clean +0/-0 │  ← 結果
│   workspace/proj-b        main  3d4e5f6  behind      clean +0/-1 │
│   workspace/proj-c        main  7g8h9i0  up-to-date  dirty  s:2  │  ← s:N = stash 数
│   …                                                              │
├──────────────────────────────────────────────────────────────────┤
│ j/k/g/G move  PgUp/PgDn page  / filter  s sort  ? help  q quit   │  ← ヘルプバー
╰──────────────────────────────────────────────────────────────────╯
```

ティッカーは**データ駆動**です — スキャン実行中に新たな `behind` / `ahead` / `diverged` / `dirty` リポジトリが検出されるたびに `⚡` または `⚠` アイテムがニュース速報ストリップに追加されます。3 行目はスキャン中に**プログレスバー**を表示します。

**キー操作**

| キー              | 動作                                                        |
| ----------------- | ----------------------------------------------------------- |
| `j` / ↓           | カーソルを下に移動                                          |
| `k` / ↑           | カーソルを上に移動                                          |
| `g` / `G`         | リストの先頭 / 末尾にジャンプ                               |
| `PgDn` / `PgUp`   | 1 ページ下 / 上にスクロール                                 |
| `/`               | ライブフィルタ — 入力しながら即時絞り込み、Enter で確定、Esc でキャンセル |
| `Esc`             | アクティブなフィルタをクリア                                |
| `Enter`           | 選択リポジトリの詳細ペインを開く                            |
| `s`               | ソートモードをサイクル: `path` → `status` → `date` → `branch` → `ahead-desc` → `behind-desc` |
| `f`               | `--fetch` フラグを切り替えて再スキャン                      |
| `F`               | `repo sync` を実行（AOSP ワークスペース専用 — `.repo/` が必要） |
| `r`               | 再スキャン（`find` とワーカーを再実行）                     |
| `T`               | `repo status` オーバーレイを表示（AOSP ワークスペース専用）  |
| `e`               | 現在の（フィルタ済み・ソート済み）ビューをファイルにエクスポート |
| `c`               | カラムヘッダ行をトグル                                      |
| `?`               | 全キーバインドのヘルプオーバーレイを表示                    |
| `q` / Ctrl-C      | 終了（端末を復元）                                          |

**カラーコード**

| カラー       | 意味          |
| ------------ | ------------- |
| 緑           | 最新          |
| 黄           | behind（遅れ）|
| シアン       | ahead（進み） |
| 赤           | diverged      |
| グレー       | upstream なし |
| 黄色 `s:N`   | stash エントリあり |

**AOSP / Google `repo` ツール ワークスペース**

スキャンルートに `.repo/` ディレクトリが存在する場合、`repo-report` は自動的に AOSP ワークスペースとして検出し、追加キーを有効化します：

- `F` — マニフェスト全体を `repo sync -j<jobs>` で同期
- `T` — `repo status` の出力をオーバーレイ表示

現在のマニフェストブランチはステータスバーに `repo:<branch>` として表示されます。非 AOSP ワークスペースではこれらのキーは無効です。

## 非インタラクティブモード

`--format`、`--non-interactive` / `-n`、または TTY でない stdout の場合に有効になります：

```sh
# 出力形式を明示的に指定
repo-report --format tsv  .  > report.tsv
repo-report --format json .  > report.json
repo-report --format table .

# TTY 上でも強制的に非インタラクティブ
repo-report -n /path/to/workspace

# パイプで stdout に流す場合は自動で TSV にフォールバック
repo-report . | awk -F'\t' 'NR>1 && $8=="behind"'

# 先にネットワーク更新（behind/ahead の正確なカウントに必要）
repo-report -j 32 --fetch --format json /path/to/workspace > report.json

# CI ゲート：dirty / behind / ahead / diverged があれば非ゼロで終了
repo-report --fetch -n . >/dev/null || echo "workspace not clean"
```

### カラム

| カラム    | 意味                                                        |
| --------- | ----------------------------------------------------------- |
| `repo`    | ワーキングツリーへのパス                                    |
| `branch`  | 現在のブランチ（HEAD がデタッチされている場合は `(detached)`）|
| `sha`     | HEAD の短縮ハッシュ                                         |
| `date`    | HEAD コミットの日付（ISO 8601）                             |
| `ahead`   | `@{u}` より HEAD が進んでいるコミット数                     |
| `behind`  | `@{u}` より HEAD が遅れているコミット数                     |
| `dirty`   | `clean` または `dirty`（`git status --porcelain` による）   |
| `status`  | `up-to-date` / `behind` / `ahead` / `diverged` / `no-upstream` |
| `remote`  | `origin` の URL                                             |
| `message` | HEAD コミットの件名                                         |
| `stash`   | stash エントリ数（なければ 0）                              |

### 終了コード

- `0` — すべてのリポジトリが `clean` かつ `up-to-date`（またはアップストリームなし）
- `1` — 少なくとも 1 つのリポジトリが dirty、behind、ahead、または diverged

### 並列処理

`repo-report` はデフォルトで `nproc` ワーカーを使用し、`xargs -P` で並列化します。NUL 区切り入力を使用するため、特殊なパスも正しく処理されます。各ワーカーは単一の `PIPE_BUF` 以下の行を出力するため、Linux 上では stdout への同時書き込みがアトミックに保たれます。

`-j N` で調整できます。I/O バウンドな `--fetch` 実行では、`nproc` より大幅に高い `-j`（例：`-j 64`）を設定するとより高速になります。

### エラーコード

ユーザーが見えるエラーは `repo-report: [RRxxx] <message>` の形式で出力され、カテゴリ別にまとめられます：`RR1xx` 引数解析、`RR2xx` ファイルシステム、`RR3xx` git/ワーカー、`RR4xx` TUI/端末、`RR5xx` 内部/依存関係。  
完全な一覧は [`docs/errors.md`](docs/errors.md) を参照してください。

### Claude Code 連携

このリポジトリには `.claude/` 以下にサブエージェントとスラッシュコマンドスキルが同梱されています：

- **`cli-reporter` エージェント**（`.claude/agents/cli-reporter.md`）—  
  このコードベースへの将来の Bash / TUI 編集に特化したエージェント。
- **`/repo-report` スキル**（`.claude/skills/repo-report/SKILL.md`）—  
  `bin/repo-report --non-interactive --format json` を実行し、Claude が 200 行のテーブルを目視確認せずに「リポジトリの状態は？」という質問に答えられるよう結果をまとめます。
