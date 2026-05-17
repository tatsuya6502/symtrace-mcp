# symtrace-mcp

[![DeepWiki][deepwiki-badge]][deepwiki]
[![GitHub Actions][gh-actions-badge]][gh-actions]

`symtrace-mcp`はAIコーディングエージェントとLanguage Server Protocolの橋渡しを行うMCP（Model Context Protocol）サーバーです。AIツールに代わって言語サーバープロセスを管理し、find-references、goto-definition、call-hierarchyトラバーサルなどの操作をstdio経由のMCPツールとして公開します。

**遅延起動**と**自動アイドルシャットダウン**により、重い言語サーバープロセスはAIエージェントが深いコード解析を要求したときだけリソースを消費します。

`ast-outline`などの既存の軽量コード解析ツールを置き換えるものではなく、補完することを目的としています。

## symtrace-mcp と ast-outline の使い分け

[`ast-outline`][ast-outline]はほとんどのコード探索ニーズをカバーしており、まず最初に使うべきツールです。`symtrace-mcp`は稼働中の言語サーバーが必要な操作を担当します。

| タスク | ツール |
|------|------|
| 構造の概要、シグネチャ、シンボル本体 | `ast-outline outline` / `show` |
| 実装の検索（tree-sitter） | `ast-outline implements` |
| ファイルレベルの依存グラフ | `ast-outline deps` / `reverse-deps` |
| セマンティック検索とBM25検索 | `ast-outline search` |
| **シンボルレベルの参照検索** | **`symtrace-mcp find_references`** |
| **Rust トレイト実装の解決** | **`symtrace-mcp find_implementations`** |
| **定義へのジャンプ（型解決付き）** | **`symtrace-mcp goto_definition`** |
| **コール階層** | **`symtrace-mcp incoming_calls`** / **`outgoing_calls`** |
| **型情報 / ホバー** | **`symtrace-mcp hover`** |
| **プル診断** | **`symtrace-mcp diagnostics`** |
| **リネームプレビュー** | **`symtrace-mcp rename`** |

`symtrace-mcp`への最初のツール呼び出しでバックグラウンドで言語サーバーが起動します。以降の呼び出しは稼働中のサーバーを再利用します。サーバーは10分間操作がないと自動的にシャットダウンします。

[ast-outline]: https://github.com/aeroxy/ast-outline

## 対応言語

symtrace-mcpはLanguage Server Protocolを介して言語サーバーと通信するため、LSP準拠のサーバーがあれば任意の言語に対応できます。

| 言語 | 言語サーバー | ステータス |
|----------|----------------|--------|
| Rust | [rust-analyzer](https://rust-analyzer.github.io/) | 対応済み |
| TypeScript / JavaScript | [typescript-language-server](https://github.com/typescript-language-server/typescript-language-server) | 計画中 |
| Python | [pyright](https://github.com/microsoft/pyright) | 計画中 |

言語サポートはプロジェクトごとに設定します。将来的には、言語サーバーのエントリを追加するだけで新しい言語に対応できるようにすることを目指しています。

## マルチプロジェクト対応

symtrace-mcpは単一リポジトリ内の複数の独立したプロジェクトを管理でき、それぞれに専用の言語サーバーインスタンスが割り当てられます。Claude Codeを起動するディレクトリに`.symtrace.toml`ファイルを作成してください：

```toml
[server.rust]
command = "rust-analyzer"

[[projects]]
root = "project-a"

[[projects]]
root = "project-b"
```

- `[[projects]]` — プロジェクトルートディレクトリのリスト（設定ファイルからの相対パス）。それぞれに専用の言語サーバーが割り当てられます。
- `[server.rust]` — グローバルな言語サーバー設定。省略可能。デフォルトは`rust-analyzer`、アイドルタイムアウト600秒。
- `.symtrace.toml`がない場合、サーバーは現在のディレクトリをプロジェクトルートとしてシングルプロジェクトモードで動作します。

ツール呼び出しはファイルパスに基づいて自動的に正しいプロジェクトの言語サーバーにルーティングされます（最長プレフィックスマッチ）。

## 使用統計

`symtrace-mcp`はツール呼び出しと言語サーバーのライフサイクルイベントをプロジェクトごとの[Turso](https://github.com/tursodatabase/turso)データベース（SQLite互換、`.symtrace/stats.db`）に記録します。データは30日後に自動的に削除されます。

過去7日間のサマリーを表示します：

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

Rustプロジェクトを解析する場合は、`rust-analyzer`をインストールして`PATH`に含まれるようにしてください。

```bash
rustup component add rust-analyzer
rustup component add rust-src
```

リポジトリをクローンしてサーバーをインストールします：

```bash
git clone https://github.com/tatsuya6502/symtrace-mcp.git
cd symtrace-mcp
cargo install --path .
```

AIエージェントのツール設定に`symtrace-mcp`を追加し、実行可能ファイルのパスと必要な引数を指定します。

<details>
  <summary>Claude Code example</summary>

```bash
claude mcp add --scope user symtrace-mcp -- symtrace-mcp
```

重複する言語サーバーインスタンスの起動を防ぐため、組み込みの`rust-analyzer-lsp`プラグインを無効化してください：

```bash
claude plugin disable rust-analyzer-lsp@claude-plugins-official
```

または`~/.claude/settings.json`に以下を追加してください：

```json
{
  "enabledPlugins": {
    "rust-analyzer-lsp@claude-plugins-official": false
  }
}
```

</details>

### 組み込みLSPプラグインではなくsymtrace-mcpを選ぶ理由（2026年5月時点）

2026年5月時点で、Claude Codeには[11言語に対応した組み込みLSPプラグイン](https://code.claude.com/docs/en/discover-plugins#code-intelligence)が付属しており、コードナビゲーションと自動診断を提供します。symtrace-mcpは組み込みプラグインにない機能を追加します：

| | 組み込みLSPプラグイン | symtrace-mcp |
|--|---------------------|--------------|
| マルチプロジェクト | ワークスペースごとに1つのLSP | `.symtrace.toml`で複数プロジェクト対応 |
| アイドルシャットダウン | なし — CLIプロセスの間ずっとサーバーが生存 | 10分間の非操作後に自動シャットダウン |
| 使用統計 | なし | `symtrace-mcp stats` |
| 対応ツール | Claude Codeのみ | MCP互換のAIツール全般 |
| 自動診断 | 対応 | プル診断（`diagnostics`ツール） |
| 対応言語 | 11言語（C/C++、C#、Go、Java、Kotlin、Lua、PHP、Python、Rust、Swift、TS/JS） | Rustのみ（TS/Pythonは計画中） |

Claude Codeで単一のRustプロジェクトを扱う場合、組み込みプラグインで十分かもしれません。マルチプロジェクト対応、アイドル時のリソース管理、使用統計が必要な場合や、Claude Code以外のAIツールも併用する場合にsymtrace-mcpをおすすめします。

### MCP Protocol

サーバーはstdinから改行区切りのJSON-RPC 2.0メッセージを読み取り、stdoutにレスポンスを出力します。

## ロードマップ

| 項目 | スコープ | ステータス |
|------|-------|--------|
| 基盤 | MCPプロトコル、LSPトランスポート、LSPプロセス管理 | 完了 |
| 最小機能 | `find_references`、`goto_definition`、`find_implementations` | 完了 |
| マルチプロジェクト設定 | `.symtrace.toml`、プロジェクトごとの言語サーバー | 完了 |
| コール階層 | `incoming_calls`、`outgoing_calls` | 完了 |
| 使用統計 | ツール呼び出し追跡、統計CLI、SQLite互換ストレージ | 完了 |
| マルチ言語 | TypeScriptおよびPython対応 | 計画中 |
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
