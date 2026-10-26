# symtrace-mcp

[![DeepWiki][deepwiki-badge]][deepwiki]
[![GitHub Actions][gh-actions-badge]][gh-actions]

`symtrace-mcp`は、AIコーディングエージェントとLanguage Server Protocolの橋渡しをするMCP（Model Context Protocol）サーバーです。AIツールに代わって言語サーバーのプロセスを管理し、find-references、goto-definition、call-hierarchyトラバーサルなどの操作を、stdio経由のMCPツールとして公開します。

**遅延起動**と**自動アイドルシャットダウン**により、重い言語サーバーのプロセスは、AIエージェントが深いコード解析を求めたときだけリソースを使います。

[`ast-bro`][ast-bro]などの既存の軽量なコード解析ツールを置き換えるものではなく、補うことを目的としています。

## symtrace-mcp と ast-bro の使い分け

コード探索の大半は[`ast-bro`][ast-bro]（`sb`）で足りるので、まずはこちらを使ってください。`symtrace-mcp`は、稼働中の言語サーバーが必要な操作を受け持ちます。

| タスク | ツール |
|------|------|
| 構造の概要、シグネチャ、シンボル本体 | `sb map` / `digest` / `show` |
| 実装の検索（tree-sitter） | `sb implements` |
| ファイルレベルの依存グラフ | `sb deps` / `reverse-deps` |
| コールグラフ（tree-sitterによるヒューリスティック、型解決なし） | `sb callers` / `callees` / `impact` |
| セマンティック検索とBM25検索 | `sb search` |
| **シンボルレベルの参照検索** | **`symtrace-mcp find_references`** |
| **Rust トレイト実装の解決** | **`symtrace-mcp find_implementations`** |
| **定義へのジャンプ（型解決付き）** | **`symtrace-mcp goto_definition`** |
| **コール階層（型解決付き）** | **`symtrace-mcp incoming_calls`** / **`outgoing_calls`** |
| **型情報 / ホバー** | **`symtrace-mcp hover`** |
| **プル診断** | **`symtrace-mcp diagnostics`** |
| **リネームプレビュー** | **`symtrace-mcp rename`** |

`symtrace-mcp`への最初のツール呼び出しで、言語サーバーがバックグラウンドで起動します。以降の呼び出しは稼働中のサーバーを再利用します。サーバーは10分間操作がないと自動的にシャットダウンします。

[ast-bro]: https://github.com/aeroxy/ast-bro

## 対応言語

symtrace-mcpはLanguage Server Protocolで言語サーバーと通信するため、LSP準拠のサーバーがあればどの言語にも対応できます。

| 言語 | 言語サーバー | ステータス |
|----------|----------------|--------|
| Rust | [rust-analyzer](https://rust-analyzer.github.io/) | 対応済み |
| TypeScript / JavaScript | [typescript-language-server](https://github.com/typescript-language-server/typescript-language-server) | 対応済み |
| Python | [pyright](https://github.com/microsoft/pyright) | 計画中 |

言語サーバーは、`.symtrace.toml`の言語ごとの`[server.<lang>]`セクションで設定します（[設定](#設定)を参照）。言語サーバーのエントリーを追加するだけで、コードを変更せずに新しい言語に対応できるようにすることを目指しています。

## 設定

symtrace-mcpは、AIエージェントを起動したディレクトリにある`.symtrace.toml`を読み込みます。このファイルは省略できます。ない場合は、現在のディレクトリをプロジェクトルートとし、RustとTypeScriptのデフォルトのサーバー設定を使って、シングルプロジェクトモードで動作します。

```toml
[server.rust]
# command defaults to rust-analyzer when omitted
index_timeout_secs = 120  # raise for large workspaces

[server.typescript]
command = "typescript-language-server"
idle_timeout_secs = 300

[logging]
level = "debug"

[[projects]]
root = "project-a"

[[projects]]
root = "project-b"
env = { DATABASE_URL = "postgres://localhost/mydb" }
```

| キー | デフォルト | 説明 |
|-----|---------|-------------|
| `[server.<lang>]` | RustとTypeScriptのデフォルト設定 | 言語ごとのサーバー設定。`<lang>`は`rust`か`typescript`です。中のキーはすべて省略できるので、1つだけ書いたセクションでもかまいません |
| `command` | `rust-analyzer` / `typescript-language-server --stdio` | 言語サーバーの起動コマンド。TypeScriptのデフォルト設定はtypescript-language-serverを前提にしています。vtslsやdenoなどほかのサーバーは初期化オプション`tsserver.useSyntaxServer = "never"`を無視するため、最初のクエリーに、読み込みが終わっていないプロジェクトの情報で応答します |
| `idle_timeout_secs` | `600` | ツール呼び出しがないまま、この秒数が過ぎると言語サーバーをシャットダウンします。次の呼び出しで再び起動します |
| `index_timeout_secs` | `30` | 最初のツール呼び出しで、言語サーバーがワークスペースを読み込み終えるまで待つ時間。`0`にすると待ちません。上限は`86400`です |
| `[logging] level` | `info` | ログレベル。`off`、`error`、`warn`、`info`、`debug`、`trace`のほか、`symtrace_mcp::lsp=debug`のような[`tracing-subscriber`のTargetsフィルター](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.Targets.html)も指定できます |
| `[[projects]]` `root` | 現在のディレクトリ | プロジェクトルート（設定ファイルからの相対パス）。プロジェクトごとに、言語ごとの言語サーバーインスタンスが割り当てられます |
| `[[projects]]` `env` | なし | このプロジェクトの言語サーバーに渡す環境変数。親プロセスの環境に追加されます |

### マルチプロジェクト対応

`[[projects]]`を複数書くと、symtrace-mcpは1つのリポジトリ内の独立した複数のプロジェクトを管理し、それぞれに専用の言語サーバーインスタンスを割り当てます。ツール呼び出しは、ファイルパスをもとに正しいプロジェクトへ振り分けられます（最長プレフィックスマッチ）。

### インデックスのタイムアウト

タイムアウトしても、サーバーはそのまま使われます。rust-analyzerがまだインデックスを作っている間は、空の応答が「language server is still indexing」エラーとして返るので、エージェントは再試行できます。`index_timeout_secs = 0`にすると、言語サーバーがインデックス作成の状況を報告する前に最初のクエリーが実行されることがあり、その結果は不完全になる場合があります。このエラーは、その状況報告をもとに判定しているためです。

### ログ

実行のたびに、JSONL形式のログファイルが`.symtrace/logs/`に書き出されます（ファイル名は`symtrace-mcp.<timestamp>.<pid>.log`）。7日より古いファイルは起動時に削除されます。次の2つの環境変数を使うと、設定ファイルを編集せずにログの出し方を変えられます。

- `SYMTRACE_LOG`：ログレベル。`[logging] level`より優先されます。`SYMTRACE_LOG=off`にするとファイルへのログ出力を止めます
- `SYMTRACE_LOG_DIR`：`.symtrace/logs/`の代わりに使うログディレクトリ

## 使用統計

`symtrace-mcp`は、ツール呼び出しと言語サーバーのライフサイクルイベントを、プロジェクトごとの[Turso](https://github.com/tursodatabase/turso)データベース（SQLite互換、`.symtrace/stats.db`）に記録します。データは30日後に自動的に削除されます。

過去7日間のサマリーを表示するには、次のコマンドを実行します。

```bash
symtrace-mcp stats
```

出力例：

```text
Usage Stats (last 7 days)

Tool Usage:
  goto_definition            32 calls   89ms avg    2 errors
  find_references            18 calls   45ms avg    0 errors
  find_implementations        8 calls  120ms avg    1 errors
  incoming_calls              5 calls   67ms avg    0 errors
  outgoing_calls              3 calls   52ms avg    0 errors

Top Files:
  src/mcp/tools.rs                              28 calls
  src/server/manager.rs                         15 calls
  src/main.rs                                    8 calls

Language Servers:
  rust        started  3×  avg startup  2.3s  uptime 4h 12m total
```

データがまだ収集されていない場合：

```text
No stats data found.
```

## インストール

### 前提条件

使う言語の言語サーバーをインストールしてください。

```bash
# Rust
rustup component add rust-analyzer
rustup component add rust-src

# TypeScript / JavaScript
npm install -g typescript-language-server typescript

# Claude Code：組み込みのrust-analyzerプラグインを無効化して、
# rust-analyzerが2つ同時に動かないようにする
claude plugin disable rust-analyzer-lsp@claude-plugins-official
```

最後の手順は、`~/.claude/settings.json`の`enabledPlugins`に`"rust-analyzer-lsp@claude-plugins-official": false`を追加しても同じ結果になります。

### サーバーのインストール

最新リリースを、CargoでGitHubから直接インストールします。

```bash
cargo install --locked --git https://github.com/tatsuya6502/symtrace-mcp.git --tag v0.7.0
```

`--tag`を省くと、`main`ブランチをビルドします。インストールされたバージョンは`symtrace-mcp --version`（または`-V`）で確認できます。

Claude Code以外のAIエージェントでは、エージェントのMCPサーバー設定に`symtrace-mcp`をstdioサーバーとして追加します。コマンドは`symtrace-mcp`で、引数は要りません。

### Claude Codeとの連携

Claude Codeのセッションを開いていない状態で、`setup`を一度実行します。

```bash
symtrace-mcp setup
```

オプション：

- `--local`：ユーザー単位のファイルではなく、プロジェクトのファイル（`./CLAUDE.md`、`./.claude/agents/Explore.md`、`./.claude/settings.json`、`./.mcp.json`）を対象にします
- `--dry-run`：ファイルには書き込まず、変更内容だけを表示します
- `--skip-prompt` / `--skip-explore` / `--skip-hook` / `--skip-mcp`：その項目には手を付けません
- `--force`：手で編集したブロックやMCPエントリーを置き換えます（指定しないと、`setup`は置き換えを拒否し、0以外の終了コードで終了します）
- `--status`：項目ごとに`installed v<version>` / `outdated v<version>` / `modified` / `absent`を表示します。フックとMCPのエントリーはバージョンを記録しません。そのため、コマンドが素の`symtrace-mcp`か実行中のバイナリへのパスであれば、実行中のバイナリのバージョンを表示し、別の場所にインストールされたものを指していれば、バージョンなしの`installed`を表示します
- `--uninstall`：`setup`が追加したものだけを削除します。それらのファイルにあるほかの内容は残ります

`setup`は、ユーザー単位のClaude Codeのファイルに次の4つをインストールします。

| 項目 | ファイル | 内容 |
|------|------|--------------|
| prompt | `~/.claude/CLAUDE.md` | マーカーブロック（`<!-- symtrace-mcp:begin v=<version> -->` … `<!-- symtrace-mcp:end -->`）。意味解析が必要な質問（呼び出し元、実装、定義、参照、型）では、`sb`/`grep`より先にsymtrace-mcpのツールを使うようモデルに指示します |
| explore | `~/.claude/agents/Explore.md` | 同じブロック。組み込みのExploreサブエージェントは`CLAUDE.md`を読まないためです。ファイルがなければ、組み込みのExploreと同じfrontmatterに`disallowedTools: Write, Edit, NotebookEdit`を加えて作成します。ファイルがすでにある場合（ast-broが作ったものなど）は、ブロックだけを追加します。そのファイルのfrontmatterに`tools:`/`disallowedTools:`の行がなければ、`setup`は警告を出します。Exploreという名前のユーザーエージェントは組み込みの読み取り専用エージェントに取って代わり、すべてのツールを使えるようになってしまうためです |
| hook | `~/.claude/settings.json` | `Bash`と`Grep`のマッチャーに対する`PreToolUse`エントリー。`symtrace-mcp hook`を実行します |
| mcp | `~/.claude.json` | `mcpServers.symtrace-mcp`エントリー |

これが必要な理由：Claude Codeのオートモードでは、モデルはBashを優先して使います。そのため「Xを呼んでいるのはどこか」を調べるときは`sb callers`や`grep`を使い、`sb`を名指しした指示がない限り、MCPツールを選びません。マーカーブロックがこれを直し、フックがそれを後押しします。

フックは`Bash`と`Grep`の呼び出しを1つずつ調べます。呼び出しが`sb callers|callees|implements|impact|trace`、ソースコードを対象にした識別子だけの`grep`/`rg`、識別子だけを渡した`Grep`ツールの呼び出しのどれかであれば、呼び出しはそのまま実行させ、対応するsymtrace-mcpツールの名前を示すリマインダーを添えます。呼び出しをブロックすることはありません。どのディレクトリをソースコードとみなすかは、`SYMTRACE_HOOK_SRC_DIRS="src|crates"`のように設定して変えられます。

symtrace-mcpをアップグレードしたあとに`setup`を再実行すると、ブロックが新しいバージョンに更新されます。フックとMCPのエントリーはコマンド名`symtrace-mcp`だけを書くので、Claude Codeから見える`PATH`上にバイナリが必要です。`setup`の前に、上の`cargo install`コマンドを実行してください。`PATH`で見つかるバイナリが`setup`を実行しているものと違う場合（たとえば`target/debug/symtrace-mcp`を実行していて、`PATH`上には古いものがある場合）は、`setup`が警告を出します。`hook`サブコマンドのない古いバイナリが使われると、Claude Codeは`Bash`と`Grep`の呼び出しをすべてブロックしてしまいます。

### 組み込みLSPプラグインではなくsymtrace-mcpを選ぶ理由（2026年5月時点）

2026年5月時点で、Claude Codeには[11言語に対応した組み込みLSPプラグイン](https://code.claude.com/docs/en/discover-plugins#code-intelligence)が付属しており、コードナビゲーションと自動診断を提供します。symtrace-mcpは、組み込みプラグインにない機能を追加します。

| | 組み込みLSPプラグイン | symtrace-mcp |
|--|---------------------|--------------|
| マルチプロジェクト | ワークスペースごとに1つのLSP | `.symtrace.toml`で複数プロジェクトに対応 |
| アイドルシャットダウン | なし — CLIプロセスが動いている間、サーバーは動き続ける | 10分間操作がないと自動でシャットダウン |
| 使用統計 | なし | `symtrace-mcp stats` |
| 対応ツール | Claude Codeのみ | MCP互換のAIツール全般 |
| 自動診断 | 対応 | プル診断とプッシュ診断（`diagnostics`ツール） |
| 対応言語 | 11言語（C/C++、C#、Go、Java、Kotlin、Lua、PHP、Python、Rust、Swift、TS/JS） | Rust、TypeScript/JS（Pythonは計画中） |

Claude Codeで単一のRustプロジェクトだけを扱うなら、組み込みプラグインで足りるかもしれません。マルチプロジェクト対応、アイドル時のリソース管理、使用統計が必要な場合や、Claude Code以外のAIツールも使う場合は、symtrace-mcpが役に立ちます。

### MCP Protocol

サーバーはstdinから改行区切りのJSON-RPC 2.0メッセージを読み取り、stdoutにレスポンスを書き出します。

## ロードマップ

| 項目 | スコープ | ステータス |
|------|-------|--------|
| 基盤 | MCPプロトコル、LSPトランスポート、LSPプロセス管理 | 完了 |
| 最小機能 | `find_references`、`goto_definition`、`find_implementations` | 完了 |
| マルチプロジェクト設定 | `.symtrace.toml`、プロジェクトごとの言語サーバー | 完了 |
| コール階層 | `incoming_calls`、`outgoing_calls` | 完了 |
| 使用統計 | ツール呼び出しの記録、統計CLI、SQLite互換ストレージ | 完了 |
| マルチ言語 | TypeScript対応 | 完了 |
| マルチ言語 | Python対応 | 計画中 |
| マルチ言語 | Kotlin対応 | 計画中 |
| 高度な機能 | `hover`、`diagnostics`、`rename` | 完了 |
| インストーラとアップグレード | `curl \| sh`インストーラ、Homebrewタップ、`symtrace-mcp upgrade` | 計画中 |
| 診断コマンド | `symtrace-mcp doctor` — 環境チェックと前提条件の検証 | 計画中 |
| 言語別統計 | 言語ごとの使用統計、スキーママイグレーション | 計画中 |

## ライセンス

[MIT](LICENSE)

[deepwiki-badge]: https://deepwiki.com/badge.svg
[gh-actions-badge]: https://github.com/tatsuya6502/symtrace-mcp/workflows/Test/badge.svg

[deepwiki]: https://deepwiki.com/tatsuya6502/symtrace-mcp
[gh-actions]: https://github.com/tatsuya6502/symtrace-mcp/actions?query=workflow%3ATest
