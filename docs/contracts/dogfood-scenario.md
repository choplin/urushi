# agentlog dogfood 受け入れ契約

## 契約の目的

この契約は、urushi の MVP を合成 showcase ではなく実在する下流 CLI で検証する手順を固定する。

対象には別 repository の `agentlog` crate を採用する。
agentlog は AI coding agent のローカルな session history を同期して閲覧する製品であり、通常の CLI command、cliclack を使う確認、ratatui の catalog browser をすでに持つ。
M4 は既存の三つの表示面へ同じ application Theme を適用し、agentlog に新しい質問、設定、command option を追加しない。

三つの表示面を一つの invocation へ接続する必要はない。
通常出力は `sync`、Prompt は `purge`、TUI は `browse` という既存の利用目的を保ったまま、同じ Theme definition を process ごとに構築して使う。
Input と Select は `urushi-prompt` 自身の自動 test と example で検証し、実在しない利用目的を agentlog に作らない。

この文書で「M4で追加」と記した挙動を現行機能として扱ってはならない。

## 採用判断の根拠

agentlog の採用判断は、調査時点の main `a930fd9aabbb1b11698b0aba9c6e6a04dc0299cf` に基づく。

| 確認事項 | 現状 | 根拠 |
| --- | --- | --- |
| 実在する利用目的 | AI coding agent の session history を local catalog へ同期して閲覧する | `README.md:3-6`、`Cargo.toml:7` |
| 通常 CLI | `paths`、`sync`、`list`、`show` が human-readable または JSON を出力する | `src/cli.rs:35-65`、`src/cli.rs:275-345`、`src/cli.rs:526-599` |
| 現行 Prompt | `purge` が全 standard stream と foreground terminal を検査し、cliclack の default-No Confirm を実行する | `src/cli.rs:101-138`、`src/cli.rs:191-249` |
| 現行 TUI | `browse` が既存 catalog を表示し、background sync、refine、preview、diagnostics を提供する | `README.md:28-46`、`src/tui.rs:54-101`、`src/tui.rs:1168-1295` |
| ratatui 利用 | ratatui と crossterm を直接依存に持ち、stderr の alternate screen へ描画する | `Cargo.toml:12-15`、`src/tui.rs:14-27`、`src/tui.rs:1943-2003` |
| 現在の style 重複 | TUI が `Yellow`、`Black`、`Cyan`、`Green` と modifier を描画箇所で直接指定する | `src/tui.rs:1548-1597`、`src/tui.rs:1663-1702`、`src/tui.rs:1860-1901` |
| 非 TTY の既存保護 | `purge` Prompt は利用不可なら preview-only になり、`sync` progress は plain stderr へ縮退する | `README.md:98-106`、`src/cli.rs:191-249`、`src/cli.rs:379-516` |
| 機械可読出力の分離 | `sync --json` は stdout を JSON product に保ち、progress を stderr に置く | `README.md:91-94`、`tests/purge_cli.rs:161-181` |
| terminal restore | `browse` は正常終了、setup error、Drop、panic hook で raw mode、alternate screen、cursor の復元を試みる | `README.md:71-74`、`src/tui.rs:1943-2045` |

agentlog は三つの表示面をすでに持ち、現行 TUI の色定義重複も観察できるため、Theme 再利用の MVP 仮説を検証する対象として適合する。
一方、Input と Select を必要とする既存操作はないため、この二つの field を agentlog の dogfood 合格条件には含めない。

## 現行 command と M4 の到達状態

| command | 現行挙動 | M4 の到達状態 |
| --- | --- | --- |
| `agentlog paths` | Agentlog-owned path と provider-source boundary を stdout に出す。`--json` を持つ | human-readable 出力だけを共有 Theme へ移し、JSON は装飾しない |
| `agentlog sync` | progress と summary を stderr に出す。TTY かつ利用可能な terminal control では cliclack、非 TTY、`TERM=dumb`、`NO_COLOR` では plain report を使う。`--json` の product は stdout に残す | 共通 constructor で application Theme を一度構築し、status、success、warning、error、muted text を通常出力へ適用する。stdout と stderr の分離は維持する |
| `agentlog list` | catalog session を stdout に一行ずつ出す。`--json` を持つ | human-readable 出力だけを共有 Theme へ移し、JSON は装飾しない |
| `agentlog show` | session metadata と transcript preview を stdout に出す。`--json` を持つ | human-readable label と transcript role を共有 Theme へ移し、JSON は装飾しない |
| `agentlog purge` | report の後に cliclack の default-No Confirm を行う。Prompt 不可なら preview-only、`--yes` なら非対話で実行する | cliclack Confirm を `urushi-prompt` の Confirm へ置き換える。安全条件、default-No、preview-only、`--yes` は変えない |
| `agentlog browse` | 直ちに raw mode と alternate screen を取得し、既存 catalog を表示して background sync を開始する | 起動手順と製品挙動を変えず、共有 Theme と stderr 用 profile を ratatui adapter へ渡す |

## application Theme と出力 profile の所有

M4 で追加する agentlog の application Theme は、新規 `src/theme.rs` だけが所有する。
この module は次の三項目を定義する。

```rust
pub(crate) struct AgentlogThemeExtension {
    // Agentlog-specific roles only.
}

pub(crate) const DEFAULT_COLOR_SCHEME: ColorScheme = ColorScheme::Dark;

pub(crate) fn theme_set() -> ThemeSet<AgentlogThemeExtension>;
```

`AgentlogThemeExtension` は agentlog 固有の意味だけを保持し、urushi の共通 `ComponentRole` と同義の style を複製しない。
MVP では `DEFAULT_COLOR_SCHEME` を常に使い、config、CLI option、terminal 背景の推測による scheme 選択を追加しない。
`TerminalProfile` も light と dark を選ばない。

各 process は `theme_set()` を一度だけ呼び、`ThemeSet::select(DEFAULT_COLOR_SCHEME)` で選択した `&Theme<AgentlogThemeExtension>` を process 内の consumer へ渡す。
通常出力、Prompt、ratatui は ThemeSet を再構築せず、選択済み Theme から role を解決する。
別 process である `agentlog sync`、`agentlog purge`、`agentlog browse` は同じ constructor と定数を使うため、同じ Theme definition を得る。

`TerminalProfile::detect_for` は ANSI bytes を最終的に書き込む stream ごとに呼ぶ。
human-readable な `paths`、`list`、`show` の stdout は stdout に対して検出した profile を使う。
`sync` と `purge` の report および progress、`purge` Prompt、ratatui TUI は stderr に対して検出した profile を使う。
JSON 出力は Theme と profile による装飾を通さず、既存の serialization result だけを stdout に書く。

`agentlog browse` は stderr に対して `TerminalProfile::detect_for` を一度呼び、その参照を ratatui adapter へ渡す。
`browse` は ThemeSet と profile を再構築または再検出せず、起動前に Prompt を挟まない。

## 既存 TUI の境界

M4 では `tui::run` が選択済み Theme と stderr 用 profile を借用する。
既存の catalog 取得件数、初期 grouping、startup background sync は変更しない。

```rust
pub(crate) async fn run(
    paths: &AppPaths,
    theme: &Theme<AgentlogThemeExtension>,
    profile: &TerminalProfile,
) -> anyhow::Result<()>;
```

`theme` は同じ process で `theme_set()` から選択した参照であり、`profile` は stderr に対して検出した参照である。
`tui::run` はこの二つを style 解決にだけ使い、CLI option、Prompt の回答、設定値を受け取るための新しい型を追加しない。

## 実ユーザーフロー

M4 は、同じ製品の三つの既存操作を別々に観察する。
この分離により、urushi の検証都合で agentlog の操作手順を変えずに Theme の再利用を確認できる。

### 通常出力

1. `agentlog sync` を実行する。
2. provider discovery、進捗、summary が application Theme の `Muted`、`Accent`、`Success`、`Warning`、`Error` を使うことを確認する。
3. `agentlog list` と `agentlog show` を実行し、human-readable output が共有 Theme を使い、JSON output が装飾されないことを確認する。

### Prompt

1. `--yes` を付けずに `agentlog purge` を実行する。
2. purge 対象の report を確認し、`urushi-prompt` の default-No Confirm で Enter を押す。
3. 削除が実行されず、Prompt が terminal を復元して shell へ戻ることを確認する。
4. 別 invocation で Esc または Ctrl+C を押し、cancel 後も削除されず terminal が復元されることを確認する。

### TUI

1. `agentlog browse` を実行する。
2. 追加の質問を表示せず、現行どおり catalog browser が直ちに始まることを確認する。
3. `j` または `k` で session を選び、Enter で full preview を開き、内容を確認して Esc で一覧へ戻る。
4. `q` で終了する。
5. alternate screen を離れ、raw mode を解除し、cursor を表示して shell prompt を正常に操作できることを確認する。

## TTY、NO_COLOR、非 TTY

### 色あり TTY

通常の foreground terminal で `TERM=xterm-256color` 相当かつ `NO_COLOR` を未設定にして三つの実ユーザーフローを実行する。
`ThemeSet::select(DEFAULT_COLOR_SCHEME)` で選んだ Theme から role を取得し、実際の出力 stream に対する `TerminalProfile::detect_for` と `resolve_style` を経て通常出力、Prompt、ratatui へ渡す。
consumer は色値を再指定しない。

### NO_COLOR

`NO_COLOR=1 agentlog sync`、`NO_COLOR=1 agentlog purge`、`NO_COLOR=1 agentlog browse` を実 terminal で実行する。
色の ANSI sequence は出力せず、文言、選択印、border、padding、bold など色以外の識別手段と layout を維持する。
Prompt と TUI は対話可能なままであり、`NO_COLOR` を非対話指定として扱わない。

現行実装が `NO_COLOR` で plain sync progress を選ぶのは `src/cli.rs:385-395` と `src/cli.rs:506-516` に確認できる。
現行 TUI は `NO_COLOR` を参照せず ratatui color を直接指定するため、TUI の無色化は M4 で追加する。

### 非 TTY

次を terminal から実行する。

```sh
agentlog sync --json > /tmp/agentlog-sync.json 2> /tmp/agentlog-sync.err
agentlog list > /tmp/agentlog-list.txt
agentlog purge > /tmp/agentlog-purge.out 2> /tmp/agentlog-purge.err
agentlog browse > /tmp/agentlog-browse.out 2> /tmp/agentlog-browse.err
```

`sync --json` の stdout は JSON として parse でき、stdout と stderr のどちらにも ANSI sequence を含めない。
`list` は可視文字と改行だけを出力し、ANSI sequence を含めない。
`purge` は Prompt を開始せず preview-only で終了し、削除を実行しない。
`browse` は ratatui session を開始せず、明瞭な diagnostic と非ゼロ exit status を返し、terminal state を変更しない。

## terminal lifecycle の境界

`purge` Prompt は `docs/contracts/prompt-runtime.md` の inline session を使い、alternate screen を取得しない。
正常終了、default-No、cancel、I/O error、panic の各経路で、Prompt が取得した資源の cleanup を最大一回試行する。

`browse` は Prompt runtime を経由しない。
agentlog の既存 `TerminalSession` が TUI の raw mode、alternate screen、cursor、draw、finish、Drop、panic restoration を所有し続ける。
ratatui adapter は terminal lifecycle を所有しない。

M4 は Prompt と TUI の lifecycle をそれぞれ独立して検証する。
Prompt から TUI へ引き継ぐための状態、failure seam、cleanup 順序は追加しない。
process abort、`SIGKILL`、電源断での復元は保証対象外とする。

## 同一 Theme role の適用表

agentlog は process ごとに `src/theme.rs` の `theme_set()` を一度だけ呼び、`DEFAULT_COLOR_SCHEME` で application Theme を選ぶ。
通常出力、Prompt、TUI の各 adapter は `Theme::style(role)`、`TerminalProfile::resolve_style` の順で解決し、下表の role を共有する。

| 意味または部品 | 通常出力 | purge Confirm | ratatui browse |
| --- | --- | --- | --- |
| `Body` | list/show の本文、sync summary の値 | 適用なし | session row、preview 本文 |
| `Muted` | 補助 label、件数、source metadata | 適用なし | status line、key help、metadata |
| `Accent` | provider 名、主要 heading | 適用なし | grouping header、preview heading |
| `Success` | sync 完了、purge 完了 | 適用なし | successful sync status |
| `Warning` | partial source、purge warning | 適用なし | partial source diagnostic、tool marker |
| `Error` | provider failure、command error | 適用なし | catalog error、failed source diagnostic |
| `PromptQuestion` | 適用なし | purge の question | 適用なし |
| `PromptAnswer` | 適用なし | 確定した回答 | 適用なし |
| `PromptPlaceholder` | 適用なし | 適用なし | 適用なし |
| `PromptCursor` | 適用なし | 適用なし | 適用なし |
| `PromptOption` | 適用なし | 未選択 option | refine の未選択 option |
| `PromptOptionSelected` | 適用なし | 選択 option | session selection、refine の選択 option |
| `PromptHelp` | 適用なし | 操作説明 | key help |
| `PromptError` | 適用なし | 適用なし | 適用なし |
| `Panel` | 必要な structured report だけ | 適用なし | list、preview、help、diagnostics panel |
| `PanelFocused` | 適用なし | 適用なし | active list、overlay panel |

同じ role は同じ Theme 内の一つの `Style` を参照する。
たとえば `PromptOptionSelected` を TUI selection に使うときも、ratatui 側で `Black` と `Cyan` を再指定しない。
agentlog 固有の source status で共通 role では意味が足りない場合は application extension を使い、urushi の共通 `ComponentRole` を M4 で増やさない。

## 変更可能な範囲

M4 で変更できる範囲は次に限る。

- agentlog の root `Cargo.toml` と lockfileへ、local または公開済み urushi、`urushi-prompt`、ratatui feature の dependency を追加する。
- 新規 `src/theme.rs` に `AgentlogThemeExtension`、`theme_set()`、`DEFAULT_COLOR_SCHEME` と application Theme の全定義を置く。
- `src/cli.rs` で `theme_set()` を process ごとに一度呼び、選択済み Theme と stream ごとの `TerminalProfile` を通常出力 adapter と `purge` Confirm へ渡す。
- `src/tui.rs` で Theme と `TerminalProfile` を受け取り、直接指定した ratatui の color と modifier を共有 role へ置き換える。
- 必要なら `src/display.rs` に出力 helper を置くが、application Theme の定義と constructor は置かない。
- agentlog の tests、README、手動検証手順を最終挙動に合わせる。
- urushi 側は M1 から M3 で確定した公開 API の欠陥修正に限る。
  agentlog 固有の語彙や lifecycle を汎用 API へ移さない。

M0 では `docs/contracts/theme.md`、`docs/contracts/prompt-runtime.md`、`docs/contracts/dogfood-scenario.md` だけを変更し、agentlog と urushi の製品コードを変更しない。
M1 以降の実装は、これら三契約に対する人間の明示承認後にだけ開始する。

## 対象外

- agentlog に新しい質問、設定、command option、Input、Select を追加すること。
- `browse` の取得件数、初期 grouping、startup sync、filter semantics、background sync concurrency、preview loading を変更すること。
- Prompt と ratatui を一つの invocation で連結すること。
- provider-owned log、agentlog catalog schema、sync semantics、purge の deletion boundary を変更すること。
- JSON schema または stdout と stderr の product boundary を変更すること。
- cliclack との完全な見た目互換。
- ratatui application event loop、alternate screen、background task を urushi に移すこと。
- agentlog の全 text、全 panel、全 diagnostic を一度に再設計すること。
- dynamic Form、async validation、MultiSelect、mouse support。
- synthetic catalog だけで MVP 合格と判定すること。

## 手動観察手順

### 準備

1. agentlog の revision、urushi の revision、OS、terminal emulator、`TERM`、terminal size を記録する。
2. `cargo test`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo fmt --check` を agentlog で実行し、結果を記録する。
3. provider-owned log を変更しない通常の `agentlog sync` を実行し、少なくとも一件の session が catalog にあることを `agentlog list` で確認する。
4. 実 catalog に必要な値がない場合も架空 fixture を MVP evidence にせず、利用できる provider と session を記録して手順を継続する。

### 三つの表示面

1. `NO_COLOR` を unset し、実 terminal で通常出力、Prompt、TUI の実ユーザーフローを実行する。
2. `purge` の default-No と cancel のどちらでも削除されず、Prompt 終了後に shell の echo、cursor、改行が正常であることを確認する。
3. `browse` が追加質問なしで始まり、session selection、full preview、background sync status が実 catalog の内容と一致することを確認する。
4. TUI 終了後に scrollback と shell prompt が残ることを確認する。
5. `theme_set()` の呼び出しが process ごとに一度であり、consumer ごとに color literal や同義 Style 定義が残っていないことを差分と `rg` で確認する。
6. human-readable stdout と stderr consumer が、それぞれ実際の stream に `TerminalProfile::detect_for` を適用することを確認する。

### error recovery

1. `purge` Confirm で Esc と Ctrl+C を invocation ごとに試す。
2. いずれも削除を実行せず、shell の echo と cursor が正常であることを確認する。
3. TUI を開始し、`q` で終了する。
4. test seam で Prompt と TUI の setup、draw、cleanup failure を個別に注入し、各 session が取得済み資源だけを一度 cleanup することを確認する。
5. Prompt failure が TUI の開始可否を制御する結合 test は追加しない。

### profile 別観察

1. 色あり TTY で三つの表示面を観察し、役割対応を screenshot または terminal recording と文章で記録する。
2. `NO_COLOR=1` の実 TTY で同じ操作を行い、色なしでも現在位置、warning、error を識別できることを確認する。
3. 非 TTY の四 command を実行し、JSON parse、ANSI 不在、`purge` の preview-only、`browse` の早期拒否、terminal 未変更を確認する。
4. `TERM=dumb` の `sync` が plain bounded report へ縮退することを確認する。

手動観察記録には command、exit status、stdout と stderr の保存先、目視結果、未観察項目を残す。
unit test と TestBackend の結果は回帰根拠として併記できるが、手動観察の代替にはしない。

## 合格判定

次のすべてを満たした場合だけ M4 の dogfood scenario を合格とする。

- 実在する agentlog の `sync`、`purge`、`browse` を既存の利用目的どおりに完走した。
- 三つの consumer が同じ application Theme definition を使い、同義の color と modifier を consumer 内で再定義していない。
- application Theme の所有が `src/theme.rs` に閉じ、各 process が `theme_set()` を一度だけ呼んで `DEFAULT_COLOR_SCHEME` の Theme を選んだ。
- human-readable stdout と stderr consumer が実際の出力 stream ごとに profile を検出し、JSON が装飾経路を通らなかった。
- `purge` の default-No、cancel、非 TTY の preview-only、`--yes` の既存安全契約を維持した。
- `browse` は追加質問なしで起動し、既存の取得件数、初期 grouping、startup sync、TUI 操作を維持した。
- 色あり TTY と `NO_COLOR` の実 TTY の双方で通常出力、Prompt、TUI を観察した。
- 非 TTY の JSON と text に ANSI sequence が混入せず、`purge` と `browse` が terminal state を変更する前に既存の安全挙動へ分岐した。
- Prompt と TUI の正常終了、cancel または quit、注入可能な I/O error、panic の対象経路で terminal restore を個別に検証した。
- agentlog の repository-required checks が通り、provider log、catalog semantics、JSON schema に意図しない変更がない。
- 観察結果、blocker、remediation、残余リスクが記録され、人間が MVP Outcome を明示的に承認した。

次のいずれかがあれば不合格とする。

- synthetic showcase、unit test、TestBackend だけを evidence とした。
- urushi の API を試す目的だけで agentlog に質問、設定、command option、Input、Select を追加した。
- `browse` の起動前に Prompt を挟むか、既存の取得件数、初期 grouping、startup sync を変更した。
- validation を含む Input または Select の実アプリ利用を、agentlog が検証したと報告した。
- `NO_COLOR` または非 TTY で color ANSI が残る、JSON が壊れる、対話待ちになる。
- ratatui または通常出力に共有 role と同義の color literal を残す。
- terminal restore failure を隠す、または人間の承認なしに合格とする。

## M4 の作業への割り当て

| Deliverable | この契約から渡す作業 | 完了 evidence |
| --- | --- | --- |
| 実在 CLI の通常出力を共有 Theme へ移行する | agentlog dependency、新規 `src/theme.rs`、`AgentlogThemeExtension`、`theme_set()`、`DEFAULT_COLOR_SCHEME`、`paths/sync/list/show` の human-readable output、stream ごとの profile、`NO_COLOR`、非 TTY、JSON 非装飾 | 通常利用、redirect、pipe、constructor の単一定義、既存 checks、Style 重複の差分 |
| 実在 CLI の Prompt と ratatui 画面を共有 Theme へ統合する | 完成済みの `src/theme.rs` と通常出力移行を入力として、`purge` Confirm 移行、`tui::run(paths, theme, profile)`、ratatui style 解決、Prompt と TUI の独立した lifecycle、既存 browse 挙動の維持 | purge の default-No、cancel、preview-only、browse の即時起動と操作、個別の terminal failure seam、agentlog checks |
| MVP 受け入れ検証を実施する | 本文の手動観察手順を色あり TTY、`NO_COLOR`、非 TTY で実施し、合格条件を項目ごとに判定する | command と exit status、保存した stdout と stderr、目視記録、blocker と残余リスク、人間の明示承認 |
| M4 の control ledger | 上記三つの deliverable と M4 で発見した blocker を追跡し、scenario status と次の action を管理する | 全対象 deliverable の完了、Evidence 表、MVP 成立の人間承認 |

通常出力移行を先に完了し、この作業が `src/theme.rs` と application Theme constructor を所有する。
Prompt と ratatui の統合はその完成成果を入力として直列に開始し、既存 constructor を変更せず消費する。
二つの作業で同じ `src/cli.rs` を並行編集しない。
追加の対象選定や別シナリオ設計は行わず、この文書の agentlog と手順を使う。
