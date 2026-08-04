# agentlog dogfood 受け入れ契約

## 契約の目的

この契約は、urushi の MVP を合成 showcase ではなく実在する下流 CLI で検証する手順を固定する。

対象には別 repository の `agentlog` crate を採用する。
agentlog は AI coding agent のローカルな session history を同期して閲覧する製品であり、通常の CLI command、cliclack を使う確認、ratatui の catalog browser をすでに持つ。
したがって、通常出力、Prompt、TUI の三つを同じ製品目的の中で検証できる。

ただし、agentlog の現行実装は urushi に依存せず、Input と Select を持つ連続 Prompt も存在しない。
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

agentlog は三つの表示面をすでに持ち、現行 TUI の色定義重複も観察できるため、MVP 仮説を検証する対象として適合する。
既存の `purge` は Prompt 導入箇所の実在性を示すが、catalog を消去する command なので、通常出力から TUI までを完走する主シナリオには使わない。

## 現行 command と M4 の到達状態

| command | 現行挙動 | M4 の到達状態 |
| --- | --- | --- |
| `agentlog sync` | progress と summary を stderr に出す。TTY かつ利用可能な terminal control では cliclack、非 TTY、`TERM=dumb`、`NO_COLOR` では plain report を使う。`--json` の product は stdout に残す | application Theme を一度構築し、status、success、warning、error、muted text を通常出力へ適用する。stdout と stderr の分離は維持する |
| `agentlog list` | catalog session を stdout に一行ずつ出す。`--json` を持つ | human-readable 出力だけを共有 Theme へ移し、JSON は装飾しない |
| `agentlog show` | session metadata と transcript preview を stdout に出す。`--json` を持つ | human-readable label と transcript role を共有 Theme へ移し、JSON は装飾しない |
| `agentlog purge` | report の後に cliclack の default-No Confirm を行う。Prompt 不可なら preview-only、`--yes` なら非対話で実行する | cliclack Confirm を `urushi-prompt` の Confirm へ置き換える。安全条件、default-No、preview-only、`--yes` は変えない。Input と Select は追加しない |
| `agentlog browse` | 直ちに raw mode と alternate screen を取得し、既存 catalog を表示して background sync を開始する | TUI へ入る前に Input、Select、Confirm からなる browse setup Form を追加する。Form submit 後にだけ ratatui session を開始し、同じ Theme を渡す |

`browse` の setup Form は M4 で追加する。
field は次の製品設定を収集し、既存の固定値または既存 UI 内操作へ接続する。

1. **Input**：`Maximum sessions` を文字列として受け取り、整数 `1..=500` だけを受理する。
   初期値は現行の `BROWSE_SESSION_LIMIT` と同じ `500` とする。
2. **Select**：`Initial grouping` で `Recent`、`Provider`、`Repository` のいずれかを選ぶ。
   選択肢は現行 TUI の grouping に対応させる。
3. **Confirm**：`Synchronize sources in the background?` を尋ね、現行挙動を維持するため default は yes とする。
   no なら既存 catalog だけを表示し、TUI 内の `r` による明示 sync は残す。

この Form は agentlog の汎用設定画面ではない。
動的 option、MultiSelect、provider path 設定、retention 設定は追加しない。

## 実ユーザーフロー

主シナリオは「local history を同期し、閲覧件数と初期 grouping を決め、catalog から一つの session を読む」という一つの利用目的を完了する。
M4 完了後に、実 catalog と実 terminal で次の順序を実行する。

1. `agentlog sync` を実行する。
   provider discovery、進捗、summary が通常出力として表示され、application Theme の `Muted`、`Accent`、`Success`、`Warning`、`Error` を使う。
2. `agentlog browse` を実行する。
   Prompt runtime は inline stderr を取得し、browse setup Form の Input を表示する。
3. `Maximum sessions` に `0` を入力して submit する。
   `1から500までの整数を入力してください` という validation message が `PromptError` で表示され、Form は終了せず Input に留まる。
4. 値を `50` に修正して submit する。
   validation message が消え、Select へ進む。
5. `Initial grouping` で `Provider` を選んで submit する。
6. `Synchronize sources in the background?` で default の yes を Enter により採用する。
   `ConfirmSource::Default` が記録される。
7. Prompt runtime が確定回答を残して terminal を cooked mode と visible cursor へ復元する。
8. agentlog が ratatui 用の terminal session を新しく取得し、alternate screen の `browse` を開く。
   最大五十件が provider grouping で表示され、background sync の状態が status line に現れる。
9. `j` または `k` で session を選び、Enter で full preview を開き、内容を確認して Esc で一覧へ戻る。
10. `q` で終了する。
    alternate screen を離れ、raw mode を解除し、cursor を表示して shell prompt を正常に操作できる状態へ戻す。

手順 1 と手順 2 は別 process だが、同じ application Theme definition を使う。
手順 2 から手順 10 では、一つの `agentlog browse` process が Prompt と ratatui の terminal lifecycle を順番に所有し、二つを同時に取得しない。

## cancel のシナリオ

cancel は主シナリオと別の invocation で確認する。

1. `agentlog browse` を実行し、Input、Select、Confirm のいずれかで Esc または Ctrl+C を押す。
2. `FormOutcome::Cancelled` を受けた agentlog は ratatui を開始せず、exit status 0 で戻る。
3. Prompt 領域は消去され、raw mode が解除され、cursor が表示される。
4. 部分入力は stdout、stderr、catalog、config のいずれにも公開または保存されない。
5. 続けて shell で文字入力と Enter を行い、echo、cursor、改行が正常であることを確認する。

TUI を開始した後の `q` と Browse view での Esc は、現行どおり TUI の正常終了操作である。
Prompt cancel と TUI quit を同じ状態として実装しない。

## TTY、NO_COLOR、非 TTY

### 色あり TTY

通常の foreground terminal で `TERM=xterm-256color` 相当かつ `NO_COLOR` を未設定にして主シナリオを実行する。
`ThemeSet::select` で選んだ一つの Theme から role を取得し、`TerminalProfile::resolve_style` を経て通常出力、Prompt、ratatui へ渡す。
consumer は色値を再指定しない。

### NO_COLOR

`NO_COLOR=1 agentlog sync` と `NO_COLOR=1 agentlog browse` を実 terminal で実行する。
色の ANSI sequence は出力せず、文言、選択印、border、padding、bold など色以外の識別手段と layout を維持する。
Prompt と TUI は対話可能なままであり、`NO_COLOR` を非対話指定として扱わない。

現行実装が `NO_COLOR` で plain sync progress を選ぶのは `src/cli.rs:385-395` と `src/cli.rs:506-516` に確認できる。
現行 TUI は `NO_COLOR` を参照せず ratatui color を直接指定するため、TUI の無色化は M4 で追加する。

### 非 TTY

次を terminal から実行する。

```sh
agentlog sync --json > /tmp/agentlog-sync.json 2> /tmp/agentlog-sync.err
agentlog list > /tmp/agentlog-list.txt
agentlog browse > /tmp/agentlog-browse.out 2> /tmp/agentlog-browse.err
```

`sync --json` の stdout は JSON として parse でき、stdout と stderr のどちらにも ANSI sequence を含めない。
`list` は可視文字と改行だけを出力し、ANSI sequence を含めない。
`browse` は Prompt と ratatui のどちらも開始せず、`RunError::NotInteractive` に対応する明瞭な diagnostic と非ゼロ exit status を返し、terminal state を変更しない。

`browse` の非 TTY で Prompt を省略して default 値により TUI を開始する fallback は設けない。
一方、`purge` は現行の安全契約を維持し、Prompt 不可なら preview-only で成功する。

## terminal lifecycle の境界

Prompt は `docs/contracts/prompt-runtime.md` の inline session を使い、alternate screen を取得しない。
Prompt の submit または cancel が cleanup を完了した後でだけ、agentlog の ratatui session が raw mode、alternate screen、hidden cursor を取得する。

ratatui adapter は terminal lifecycle を所有しない。
agentlog の既存 `TerminalSession` が TUI の enter、draw、finish、Drop、panic restoration を所有し続ける。
M4 では Prompt session と TUI session の境界に失敗を注入する test seam を設け、Prompt cleanup 失敗時に TUI を開始しない。

正常 submit、Prompt cancel、Prompt I/O error、TUI setup error、TUI draw error、TUI の `q`、panic の各経路で、取得した資源ごとに cleanup を最大一回試行する。
process abort、`SIGKILL`、電源断での復元は保証対象外とする。

## 同一 Theme role の適用表

agentlog は process ごとに application Theme を一度構築する。
通常出力、Prompt、TUI の各 adapter は `Theme::style(role)`、`TerminalProfile::resolve_style` の順で解決し、下表の role を共有する。

| 意味または部品 | 通常出力 | browse setup Prompt | ratatui browse |
| --- | --- | --- | --- |
| `Body` | list/show の本文、sync summary の値 | 適用なし | session row、preview 本文 |
| `Muted` | 補助 label、件数、source metadata | 適用なし | status line、key help、metadata |
| `Accent` | provider 名、主要 heading | 適用なし | grouping header、preview heading |
| `Success` | sync 完了、purge 完了 | 適用なし | successful sync status |
| `Warning` | partial source、purge warning | 適用なし | partial source diagnostic、tool marker |
| `Error` | provider failure、command error | 適用なし | catalog error、failed source diagnostic |
| `PromptQuestion` | 適用なし | 三つの question | 適用なし |
| `PromptAnswer` | 適用なし | Input の値と確定回答 | 適用なし |
| `PromptPlaceholder` | 適用なし | Input placeholder | 適用なし |
| `PromptCursor` | 適用なし | Input cursor | 適用なし |
| `PromptOption` | 適用なし | Select と Confirm の未選択 option | refine の未選択 option |
| `PromptOptionSelected` | 適用なし | Select と Confirm の選択 option | session selection、refine の選択 option |
| `PromptHelp` | 適用なし | field help と操作説明 | key help |
| `PromptError` | 適用なし | Input validation message | 入力 field がないため適用なし |
| `Panel` | 必要な structured report だけ | 適用なし | list、preview、help、diagnostics panel |
| `PanelFocused` | 適用なし | 適用なし | active list、overlay panel |

同じ role は同じ Theme 内の一つの `Style` を参照する。
たとえば `PromptOptionSelected` を TUI selection に使うときも、ratatui 側で `Black` と `Cyan` を再指定しない。
agentlog 固有の source status で共通 role では意味が足りない場合は application extension を使い、urushi の共通 `ComponentRole` を M4 で増やさない。

## 変更可能な範囲

M4 で変更できる範囲は次に限る。

- agentlog の root `Cargo.toml` と lockfileへ、local または公開済み urushi、`urushi-prompt`、ratatui feature の dependency を追加する。
- `src/cli.rs` で Theme の構築と通常出力 adapter、browse setup Form、purge Confirm を接続する。
- `src/tui.rs` で Theme と `TerminalProfile` を受け取り、直接指定した ratatui の color と modifier を共有 role へ置き換える。
- 必要なら `src/display.rs` または新しい小さな theme module に agentlog application Theme と出力 helper を置く。
- agentlog の tests、README、手動検証手順を最終挙動に合わせる。
- urushi 側は M1 から M3 で確定した公開 API の欠陥修正に限る。
  agentlog 固有の語彙や lifecycle を汎用 API へ移さない。

M0 ではこの文書だけを変更し、agentlog と urushi の製品コードを変更しない。

## 対象外

- provider-owned log、agentlog catalog schema、sync semantics、purge の deletion boundary の変更。
- agentlog の filter semantics、background sync concurrency、preview loading の再設計。
- JSON schema または stdout と stderr の product boundary の変更。
- cliclack との完全な見た目互換。
- ratatui application event loop、alternate screen、background task を urushi に移すこと。
- agentlog の全 text、全 panel、全 diagnostic を一度に再設計すること。
- dynamic Form、async validation、MultiSelect、mouse support。
- synthetic catalog だけによる MVP 合格判定。

## 手動観察手順

### 準備

1. agentlog の revision、urushi の revision、OS、terminal emulator、`TERM`、terminal size を記録する。
2. `cargo test`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo fmt --check` を agentlog で実行し、結果を記録する。
3. provider-owned log を変更しない通常の `agentlog sync` を実行し、少なくとも一件の session が catalog にあることを `agentlog list` で確認する。
4. 実 catalog に必要な値がない場合も架空 fixture を MVP evidence にせず、利用できる provider と session を記録して手順を継続する。

### 主シナリオ

1. `NO_COLOR` を unset し、実 terminal で前述の実ユーザーフローを手順 1 から 10 まで実行する。
2. Input の invalid state、修正後の遷移、Select の Provider、Confirm の default yes をそれぞれ観察結果として記録する。
3. Prompt 終了時に一度通常画面へ戻ってから alternate screen が始まり、TUI 終了後に scrollback と shell prompt が残ることを確認する。
4. `browse` の session selection、full preview、background sync status が実 catalog の内容と一致することを確認する。
5. 同一 Theme role の適用表について、consumer ごとに color literal や同義 Style 定義が残っていないことを差分と `rg` で確認する。

### cancel と error recovery

1. `agentlog browse` を再実行し、Input で Esc を押す。
2. Select と Confirm でも invocation を分けて Esc または Ctrl+C を試す。
3. いずれも TUI が開始せず、部分値が出力または保存されず、shell の echo と cursor が正常であることを確認する。
4. TUI を開始し、`q` で終了する。
5. test seam では Prompt cleanup failure と TUI setup failure を注入し、後続 session を開始せず取得済み資源だけを一度 cleanup することを確認する。

### profile 別観察

1. 色あり TTY で主シナリオを完走し、通常出力、Prompt、TUI の役割対応を screenshot または terminal recording と文章で記録する。
2. `NO_COLOR=1` の実 TTY で同じ操作を完走し、色なしでも現在位置、validation、warning、error を識別できることを確認する。
3. 非 TTY の三 command を実行し、JSON parse、ANSI 不在、`browse` の早期拒否、terminal 未変更を確認する。
4. `TERM=dumb` の `sync` が plain bounded report へ縮退することを確認する。

手動観察記録には command、exit status、stdout と stderr の保存先、目視結果、未観察項目を残す。
unit test と TestBackend の結果は回帰根拠として併記できるが、手動観察の代替にはしない。

## 合格判定

次のすべてを満たした場合だけ M4 の dogfood scenario を合格とする。

- 実在する agentlog の利用目的として、通常出力、Input、validation recovery、Select、Confirm、ratatui browse、session preview を順に完走した。
- 三つの consumer が同じ application Theme definition を使い、同義の color と modifier を consumer 内で再定義していない。
- 色あり TTY と `NO_COLOR` の実 TTY の双方で主シナリオと cancel を観察した。
- 非 TTY の JSON と text に ANSI sequence が混入せず、`browse` は terminal state を変更する前に拒否された。
- Prompt submit、Prompt cancel、TUI quit、注入可能な I/O error、panic の対象経路で terminal restore を検証した。
- agentlog の repository-required checks が通り、provider log、catalog semantics、JSON schema に意図しない変更がない。
- 観察結果、blocker、remediation、残余リスクが記録され、人間が MVP Outcome を明示的に承認した。

次のいずれかがあれば不合格とする。

- synthetic showcase、unit test、TestBackend だけを evidence とした。
- Input または Select を省略し、現行 `purge` Confirm だけで Prompt 要件を満たしたと扱った。
- validation failure で Form が終了するか、cancel 後に TUI が開始する。
- Prompt cleanup 完了前に TUI が raw mode または alternate screen を取得する。
- `NO_COLOR` または非 TTY で color ANSI が残る、JSON が壊れる、対話待ちになる。
- ratatui または通常出力に共有 role と同義の color literal を残す。
- terminal restore failure を隠す、または人間の承認なしに合格とする。

## M4 の作業への割り当て

| Deliverable | この契約から渡す作業 | 完了 evidence |
| --- | --- | --- |
| 実在 CLI の通常出力を共有 Theme へ移行する | agentlog dependency、application Theme、`sync/list/show` の human-readable output、TTY profile、`NO_COLOR`、非 TTY、JSON 非装飾 | 通常利用、redirect、pipe、既存 checks、Style 重複の差分 |
| 実在 CLI の Prompt と ratatui 画面を共有 Theme へ統合する | `browse` setup Form の Input、validation、Select、Confirm、`purge` Confirm 移行、Theme の Prompt と ratatui への受け渡し、Prompt から TUI への lifecycle 境界 | 主シナリオと cancel の実 terminal 完走、terminal failure seam、agentlog checks |
| MVP 受け入れ検証を実施する | 本文の手動観察手順を色あり TTY、`NO_COLOR`、非 TTY で実施し、合格条件を項目ごとに判定する | command と exit status、保存した stdout と stderr、目視記録、blocker と残余リスク、人間の明示承認 |
| M4 の control ledger | 上記三つの deliverable と M4 で発見した blocker を追跡し、scenario status と次の action を管理する | 全対象 deliverable の完了、Evidence 表、MVP 成立の人間承認 |

通常出力移行と Prompt・ratatui 統合は実装責務を分けるが、application Theme の所有場所と constructor signature は着手時に一度だけ合意する。
追加の対象選定や別シナリオ設計は行わず、この文書の agentlog と手順を使う。
