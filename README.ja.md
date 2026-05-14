# symtrace-mcp

> [!CAUTION]
> このプロジェクトは初期開発段階です。破壊的変更が予想されます。

AIコーディングエージェントとLanguage Server Protocolの橋渡しを行うMCP（Model Context Protocol）サーバーです。AIツールに代わって言語サーバープロセスを管理し、find-references、goto-definition、call-hierarchyトラバーサルなどの操作をstdio経由のMCPツールとして公開します。

**遅延起動**と**自動アイドルシャットダウン**により、重い言語サーバープロセスはAIエージェントが深いコード解析を要求したときだけリソースを消費します。

`ast-outline`などの既存のコード解析ツールを置き換えるものではなく、補完することを目的としています。

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
| **型情報 / ホバー** | **`symtrace-mcp hover`** *(計画中)* |

`symtrace-mcp`への最初のツール呼び出しでバックグラウンドで言語サーバーが起動します。以降の呼び出しは稼働中のサーバーを再利用します。サーバーは10分間操作がないと自動的にシャットダウンします。

[ast-outline]: https://github.com/aeroxy/ast-outline

## 対応言語

symtrace-mcpはLanguage Server Protocolを介して言語サーバーと通信するため、LSP準拠のサーバーがあれば任意の言語に対応できます。

| 言語 | 言語サーバー | ステータス |
|----------|----------------|--------|
| Rust | [rust-analyzer](https://rust-analyzer.github.io/) | 対応済み |
| TypeScript / JavaScript | [typescript-language-server](https://github.com/typescript-language-server/typescript-language-server) | 計画中 |
| Python | [pyright](https://github.com/microsoft/pyright) | 計画中 |

言語サポートはプロジェクトごとに設定します。新しい言語の追加には言語サーバーのエントリだけで済み、コードの変更は不要です。

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

`symtrace-mcp`はツール呼び出しと言語サーバーのライフサイクルイベントをプロジェクトごとのSQLiteデータベース（`.symtrace/stats.db`）に記録します。データは30日後に自動的に削除されます。

過去7日間のサマリーを表示します：

```bash
symtrace-mcp stats
```

出力例：

```
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

```
No stats data found.
```

## 現在のステータス

**フェーズ 2（コール階層）** — 完了。`incoming_calls`（呼び出し元）と`outgoing_calls`（呼び出し先）の2つのMCPツールがcallHierarchyプロトコル経由で利用可能です。

**フェーズ 1（最小機能）** — 完了。`find_references`、`goto_definition`、`find_implementations`の3つのMCPツールが利用可能です。サーバーはrust-analyzerを遅延起動し、mtime追跡付きでオープンファイルを管理し、アイドル状態のサーバーを自動的にシャットダウンします。`.symtrace.toml`によるマルチプロジェクト対応も完了しています。

**フェーズ 0（基盤）** — 完了。

**計画中のフェーズ：**

| フェーズ | スコープ |
|-------|-------|
| **フェーズ 3: マルチ言語** | TypeScriptおよびPython対応 |
| **フェーズ 4: 高度な機能** | `hover`、`diagnostics`、`rename` |

## インストール

Rustプロジェクトを解析する場合は、`rust-analyzer`をインストールして`PATH`に含まれるようにしてください。

```bash
rustup component add rust-analyzer
rustup component add rust-src
```

リポジトリをクローンしてサーバーをインストールします：

```bash
cargo install --path .
```

AIエージェントのツール設定に`symtrace-mcp`を追加し、実行可能ファイルのパスと必要な引数を指定します。

```bash
## Claude Code
claude mcp add --scope user symtrace-mcp -- symtrace-mcp
```

サーバーはstdinから改行区切りのJSON-RPC 2.0メッセージを読み取り、stdoutにレスポンスを出力します。

## ライセンス

[MIT](LICENSE)
