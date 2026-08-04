# Prompt runtime、Event、描画の契約

## 契約の目的

この契約は、`urushi-prompt` の blocking public API、Form の状態遷移、入力 backend、インライン描画、terminal lifecycle の境界を固定する。

Input、Select、Confirm は同じ runtime と状態遷移を使う。
backend 固有の型、ANSI 制御、terminal cleanup は field へ入れない。
後続実装はこの文書の型と不変条件を基準とし、非同期 runtime、Elm runtime、full-screen TUI runtime を追加しない。

## 公開実行契約

利用者が呼ぶ入口は `Form::run` だけである。
呼び出しは Form が submit、cancel、または実行エラーに到達するまで現在の thread を block する。

```rust
use urushi::{TerminalProfile, Theme};

impl Form {
    pub fn run<E>(
        self,
        theme: &Theme<E>,
        profile: &TerminalProfile,
    ) -> Result<FormOutcome, RunError>;
}

#[derive(Debug)]
pub enum FormOutcome {
    Submitted(FormValues),
    Cancelled,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum RunError {
    NotInteractive,
    Io {
        operation: IoOperation,
        source: std::io::Error,
        cleanup: Option<std::io::Error>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IoOperation {
    EnterTerminal,
    ReadEvent,
    Render,
    Cleanup,
}
```

`Form::run` は Form を消費する。
submit された値は `FormOutcome::Submitted` だけが所有し、cancel と error は部分入力を公開しない。

四つの結果を次のように区別する。

| 状況 | 公開結果 | terminal cleanup |
| --- | --- | --- |
| 全 field の検証に成功した | `Ok(FormOutcome::Submitted(values))` | 実行後に一度だけ行う |
| 利用者が cancel した | `Ok(FormOutcome::Cancelled)` | 実行後に一度だけ行う |
| field の同期 validation に失敗した | `run` は戻らず、field を invalid 状態にして再入力を待つ | session を維持する |
| terminal の取得、入力、描画、復元に失敗した | `Err(RunError::Io { .. })` | 取得済み資源を一度だけ解放する |

`NotInteractive` は event source と描画先が対話端末として利用できないことを事前検査で検出した結果である。
この場合は terminal 状態を変更せずに戻る。
入力を pipe から推測して続行したり、非対話時の既定回答を Prompt runtime が選んだりしない。

cancel は失敗ではないため `RunError` に含めない。
validation も terminal または runtime の失敗ではないため `RunError` に含めない。

## FormValues と型付き field 値

各 field は利用者が指定した一意の名前と値の型を持つ。
submit 後の `FormValues` は `FieldKey<T>` によって値を型付きで取得する。

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldKey<T> {
    // Private name and type marker.
}

impl<T> FieldKey<T> {
    pub fn new(name: impl Into<String>) -> Self;
    pub fn name(&self) -> &str;
}

impl FormValues {
    pub fn get<T: 'static>(&self, key: &FieldKey<T>) -> Option<&T>;
}
```

Input の値は `String`、Select の値は option の `T`、Confirm の値は `ConfirmAnswer` である。
同じ Form 内の field 名は一意でなければならず、重複は Form 構築時の設定エラーとする。
値の型が一致しない場合を通常の制御経路にしないため、field 自身が対応する `FieldKey<T>` を受け取る constructor を持つ。

```rust
pub struct ConfirmAnswer {
    pub value: bool,
    pub source: ConfirmSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmSource {
    Default,
    Explicit,
}
```

Confirm で Enter によって設定済み default を採用した場合だけ `ConfirmSource::Default` とする。
左右キーまたは `y`、`n` で回答してから submit した場合は、default と同じ値でも `ConfirmSource::Explicit` とする。
default がない Confirm では、明示回答まで Enter を受け付けない。

Select は一つ以上の option を構築時に要求し、空 option は設定エラーとする。
単一 option はその option を常に選択し、複数 option の next と previous は末端で循環する。
選択結果には label ではなく option が所有する型 `T` を格納する。

Form と Group は builder で組み立て、`FormBuilder::build` が全体の構成を検査する。
field は crate が提供する sealed trait とし、MVP では外部実装を受け付けない。

```rust
impl Form {
    pub fn builder() -> FormBuilder;
}

impl FormBuilder {
    pub fn group(self, group: Group) -> Self;
    pub fn build(self) -> Result<Form, FormBuildError>;
}

impl Group {
    pub fn builder() -> GroupBuilder;
}

impl GroupBuilder {
    pub fn field<F: Field + 'static>(self, field: F) -> Self;
    pub fn build(self) -> Result<Group, GroupBuildError>;
}

pub trait Field: private::Sealed {
    // Public as a bound; implementations remain crate-owned.
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormBuildError {
    EmptyForm,
    DuplicateFieldName(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroupBuildError {
    EmptyGroup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldConfigError {
    EmptyName,
    EmptyOptions,
}
```

`Input::new` と `Confirm::new` は対応する `FieldKey`、question、初期値または default を受け取る。
`Select::new` は `FieldKey<T>`、question、`Vec<SelectOption<T>>` を受け取り、option が空なら `FieldConfigError::EmptyOptions` を返す。
field 名が空の場合は各 constructor が `FieldConfigError::EmptyName` を返し、重複名は Group を束ねる `FormBuilder::build` が返す。
Group の順序と Group 内の field 順序は builder へ追加した順序で固定する。

## runtime の所有境界

runtime は次の五つの層を一方向に接続する。

```text
EventSource -> backend-independent Event -> Form reducer
                                           |       |
                                           |       -> FormOutcome
                                           v
                                      PromptView
                                           |
Theme -> TerminalProfile -> resolved Style bundle -> InlineRenderer
                                                   |
                                             TerminalSession
```

- **EventSource**：blocking で backend event を読み、backend 非依存の `Event` に変換する。
- **Form reducer**：Form、Group、field の状態を更新し、次の `PromptView` または最終結果を返す。
- **PromptView**：表示内容、意味的 role、cursor 位置だけを持つ backend 非依存の描画モデルである。
- **InlineRenderer**：解決済み `Style` と `PromptView` から現在の出力領域を書き換える。
- **TerminalSession**：raw mode と cursor の lifecycle を所有し、renderer が使う writer を提供する。

field は `EventSource`、`InlineRenderer`、`TerminalSession` を参照しない。
renderer は Form と field の状態を変更しない。
terminal session は Theme、validation、field 値を参照しない。

既定実装は process の terminal input から event を読み、stderr の対話端末へ描画する。
これにより、Prompt の表示が stdout の機械可読出力へ混ざらない。
`TerminalProfile` は stderr の能力について呼び出し側が選択済みのものを渡す。

## backend 非依存 Event

field と Form reducer が受け取る event は次の閉じた集合とする。

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    Key(KeyEvent),
    Resize { columns: u16, rows: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KeyCode {
    Char(char),
    Enter,
    Escape,
    Tab,
    BackTab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
}

pub(crate) trait EventSource {
    fn read_event(&mut self) -> std::io::Result<Event>;
}
```

crossterm などの backend event 型は `EventSource` 実装の内側でこの型へ変換する。
key release、mouse、paste、focus など MVP が扱わない backend event は field へ渡さず、source が読み飛ばす。
未知の key code は panic や暗黙の submit に変換せず、読み飛ばす。

MVP の production adapter は crossterm を使う。
crossterm は `urushi-prompt` の private implementation dependency とし、その `Event`、`KeyCode`、terminal command を public signature と public feature に現さない。
別 backend を公開する拡張点は deterministic test seam の成立後に必要性が確認された場合だけ検討する。

`Ctrl+C` は `KeyCode::Char('c')` と `control = true` に正規化し、Form 全体の cancel とする。
`Escape` も Form 全体の cancel とする。
backend が OS signal として process を強制終了させる前に、runtime が cancel event を処理できる raw mode を使う。

`Resize` は field state を変えず、同じ state から新しい幅の `PromptView` を作り直す。
columns または rows がゼロでも panic せず、最小一列として描画する。

## Form、Group、field の共有状態遷移

Form は一つ以上の Group を順番に実行し、Group は一つ以上の field を順番に実行する。
空の Form と空の Group は構築時の設定エラーとし、runtime に特別な submit 経路を作らない。

runtime が所有する状態は次の三層である。

```rust
enum FormState {
    Running { group: usize, field: usize },
    Submitted,
    Cancelled,
}

enum FieldState {
    Active,
    Invalid { message: String },
    Accepted,
}

enum FieldAction {
    Stay,
    Accept,
    Back,
    Cancel,
}
```

各 event は active field が先に処理し、`FieldAction` だけを Form reducer へ返す。
field 固有の編集と選択は `Stay`、validation に成功した submit は `Accept`、共通の戻る操作は `Back`、共通の cancel 操作は `Cancel` になる。

Form reducer は `FieldAction` を次のように遷移させる。

| 現在位置と action | 次の状態 |
| --- | --- |
| `Stay` | 現在の field を維持する |
| `Accept` かつ同じ Group に次 field がある | 現 field を `Accepted` にし、次 field を `Active` にする |
| `Accept` かつ次 Group がある | 現 field を `Accepted` にし、次 Group の先頭 field を `Active` にする |
| `Accept` かつ Form の末尾 | 全 field の値を `FormValues` に移し、`Submitted` にする |
| `Back` かつ前 field がある | 現 field の未確定編集を保持し、前 field を `Active` に戻す |
| `Back` かつ Form の先頭 | 位置と値を変更しない |
| `Cancel` | 部分値を破棄し、`Cancelled` にする |

共通操作は `Enter` で現在の field を submit、`BackTab` で前 field へ戻る、`Escape` または `Ctrl+C` で Form を cancel とする。
`Tab` は Input 内へタブ文字を入れず、`Enter` と同じ submit 操作とする。
field 固有の key と競合する場合も cancel と back を Form 全体で優先する。

前 field へ戻ると、その field の accepted value を編集可能な状態へ戻す。
戻る前に active だった後続 field の編集値は保持するが、Form の最終 submit までは公開しない。
前 field の値を変更しても後続 field の option や validator を動的に再構築しない。
動的 Form は MVP の範囲外である。

## field 固有の遷移

### Input

Input は `String` と Unicode scalar 境界の cursor index を所有する。
左右、Home、End、Backspace、Delete、文字入力は値と cursor だけを更新し、validation を実行しない。
byte の途中を cursor 位置にせず、CJK 文字を一回の削除で壊さない。

Enter または Tab で required 検査、登録順の custom synchronous validator を実行する。
最初の失敗を `FieldState::Invalid { message }` として表示し、`FieldAction::Stay` を返す。
値を編集した時点で以前の validation message を消し、次の submit で再検査する。

validator の契約は純粋な同期関数である。

```rust
pub type Validator<T> =
    Box<dyn Fn(&T) -> Result<(), ValidationError> + Send + Sync + 'static>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub message: String,
}
```

validator は terminal I/O を行わず、I/O error を validation message に変換しない。
async validation は導入しない。

### Select

Select は option 列と現在の index を所有する。
上または左は previous、下または右は next とし、末端では循環する。
文字 key による検索と動的 option loading は MVP に含めない。

Enter または Tab は現在の option を accepted value とする。
Select は構築時に空 option を拒否するため、runtime 中の validation error を持たない。

### Confirm

Confirm は yes と no を Select と同じ二つの option として描画する。
左または上は yes、右または下は no、`y` は yes、`n` は no を明示選択する。

default がある場合、初期表示は default を選択状態にするが、回答元は未確定のままにする。
利用者が選択 key を押した時点で `Explicit`、選択 key を押さず Enter または Tab で default を採用した場合は `Default` とする。
default がない状態で Enter または Tab を押しても `Stay` とし、明示回答を促す help を表示する。

## validation と error の境界

validation は入力値を利用者が修正できる domain result であり、runtime を終了させない。
`ValidationError` は field の `PromptView` に `PromptError` role で現れ、`RunError` の source にはならない。

terminal または backend の失敗は利用者が field を編集して回復する対象ではない。
event read、render、terminal 制御が失敗した時点で reducer を停止し、cleanup 後に `RunError::Io` を返す。

validator が panic した場合は validation failure に変換しない。
panic cleanup の規則に従って terminal を復元し、元の panic を継続する。

## PromptView と inline renderer

Form reducer は ANSI 文字列ではなく、構造化した `PromptView` を生成する。
各 span は文字列と `ComponentRole` を持ち、cursor は可視文字の列位置で表す。

```rust
pub(crate) struct PromptView {
    pub lines: Vec<ViewLine>,
    pub cursor: Option<ViewCursor>,
}

pub(crate) struct ViewLine {
    pub spans: Vec<ViewSpan>,
}

pub(crate) struct ViewSpan {
    pub text: String,
    pub role: urushi::ComponentRole,
}

pub(crate) struct ViewCursor {
    pub row: u16,
    pub column: u16,
}

pub(crate) trait Renderer {
    fn draw(&mut self, view: &PromptView) -> std::io::Result<()>;
    fn finish(&mut self, outcome: RenderFinish) -> std::io::Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderFinish {
    Submitted,
    Cancelled,
    Error,
    Panicking,
}
```

renderer は開始時の terminal cursor 位置を Prompt 領域の原点とし、前回描画した行数を記録する。
再描画では原点へ戻り、必要な行を消去して `PromptView` を描き直し、前回より短くなった余剰行も消去する。
terminal 全画面の clear、scrollback の削除、alternate screen への移動は行わない。

`PromptView` の幅計算には urushi の ANSI-aware、CJK-aware な可視幅規則を使う。
狭い幅では active Input の cursor を含む水平 window を優先し、span を列幅で切り詰める。
Select option、help、validation message は利用可能な幅で折り返し、画面高を超える場合は active field とその error を優先する。
`Resize` 後は reducer の状態を変えずにこの規則で再描画する。

Input 編集中は renderer が `ViewCursor` の位置へ terminal cursor を置いて表示する。
cursor を持たない Select と Confirm の描画中は cursor を隠す。
draw の途中では cursor を隠し、flush に成功する直前に最終位置と可視性を適用するため、部分描画によるちらつきを抑える。

正常 submit の `finish` は確定した回答を最終表示として残し、その末尾の次行へ cursor を移す。
cancel の `finish` は Prompt 領域を消去して原点へ戻し、次行へ進めない。
error の `finish` は安全に消去できる取得済み Prompt 領域だけを消去する。

## Theme と TerminalProfile の境界

renderer は `docs/contracts/theme.md` の解決順をそのまま使う。

```text
Theme::style(ComponentRole)
    -> TerminalProfile::resolve_style
    -> Prompt style bundle
    -> InlineRenderer
```

Input、Select、Confirm は `Color` または `Style` を構築しない。
Form 実行開始時に renderer が必要な `ComponentRole` を Theme から一度解決し、同じ `TerminalProfile` を一度だけ適用して style bundle を作る。
再描画と field 遷移は解決済み style bundle を共有する。

Input は `PromptQuestion`、`PromptAnswer`、`PromptPlaceholder`、`PromptCursor`、`PromptHelp`、`PromptError` を使う。
Select はこれらに `PromptOption` と `PromptOptionSelected` を加える。
Confirm は yes と no を `PromptOption` と `PromptOptionSelected` で描画する。
Prompt 独自の色表、field ごとの色値、terminal capability 分岐を追加しない。

`TerminalProfile` は style の色縮退と `NO_COLOR` および非 TTY の出力方針だけを所有する。
raw mode、cursor、event、viewport、line clear は `TerminalProfile` の責務ではない。

## TerminalSession の所有権

`Form::run` の既定 session は、対話端末が通常の cooked mode で cursor を表示している状態を precondition とする。
同じ terminal に対する prompt session の入れ子と、別 thread からの同時描画はサポートしない。

session は次の順序で資源を取得する。

1. input と stderr が対話端末であることを確認する。
2. raw mode を有効にする。
3. Prompt 領域の原点を記録する。
4. 初回描画に必要な cursor 可視性を設定する。

alternate screen は取得しない。
呼び出し前に current screen が main screen でも alternate screen でも、session は画面を切り替えず、current screen の inline 領域だけを使う。
したがって alternate screen の enter と leave は常に呼び出し側の所有であり、`urushi-prompt` の cleanup 対象ではない。

session が取得した資源は逆順に解放する。

1. renderer が Prompt 領域を outcome に応じて finish する。
2. cursor を表示状態にし、安全な最終位置へ移す。
3. raw mode を無効にして cooked mode に戻す。
4. writer を flush する。

cursor の事前可視性を portable に問い合わせられないため、この契約の復元状態は「cursor visible」である。
cursor を非表示に保つ必要がある埋め込み側は、blocking public API の外側で自分の cursor policy を再適用する。

## normal、cancel、error、panic の cleanup

terminal 資源は取得直後に session guard へ登録する。
cleanup は idempotent な一つの method に集約し、明示終了と `Drop` が同じ method を呼ぶ。
各資源は取得済み flag を持ち、復元 command は最大一回だけ実行する。

| 終了経路 | cleanup の実行 | cleanup error の扱い |
| --- | --- | --- |
| normal submit | 明示 cleanup | cleanup だけが失敗した場合は `IoOperation::Cleanup` を返す |
| cancel | 明示 cleanup | cleanup だけが失敗した場合は cancel より安全を優先して `RunError::Io` を返す |
| event または render error | 明示 cleanup | primary error を `source`、cleanup error を `cleanup` に保持する |
| panic | guard の `Drop` | best effort で復元し、cleanup error で元の panic を置き換えない |

raw mode の無効化と cursor の表示は、renderer の finish が失敗しても試行する。
一つの cleanup step が失敗しても残りの step を続行し、最初の cleanup error を報告する。
panic を `catch_unwind` で `RunError` に変換しない。

process abort、`SIGKILL`、電源断では Rust の `Drop` が実行されないため、この契約は復元を保証できない。
通常の unwind、cancel、返却可能な I/O error では cleanup を必ず試行する。

## deterministic test seam

実 runtime は具象 terminal library を直接呼ばず、次の三つの trait を内部依存として受け取る。

```rust
pub(crate) trait EventSource {
    fn read_event(&mut self) -> std::io::Result<Event>;
}

pub(crate) trait Renderer {
    fn draw(&mut self, view: &PromptView) -> std::io::Result<()>;
    fn finish(&mut self, outcome: RenderFinish) -> std::io::Result<()>;
}

pub(crate) trait TerminalControl {
    fn is_interactive(&self) -> bool;
    fn enable_raw_mode(&mut self) -> std::io::Result<()>;
    fn show_cursor(&mut self) -> std::io::Result<()>;
    fn disable_raw_mode(&mut self) -> std::io::Result<()>;
    fn flush(&mut self) -> std::io::Result<()>;
}
```

production constructor は同じ backend の event source、inline renderer、terminal control を組み合わせる。
test constructor は script 化した event source、記録 renderer、記録 terminal control を注入する。
test seam は `pub(crate)` とし、MVP の public API に backend 選択を公開しない。

疑似 event source は `Event` または指定位置の `io::Error` を順番に返す。
疑似 renderer は全 `PromptView` と `finish` を保存し、指定回の draw または finish を失敗させられる。
疑似 terminal は全 lifecycle call を順番に保存し、各 call を失敗させられる。

最低限、次の挙動を端末なしで検証する。

- Input、Select、Confirm が同じ event loop で順に進み、back 後に値を編集して submit できる。
- validation failure が runtime を終了せず、error view の後に修正して進める。
- cancel が `FormOutcome::Cancelled` を返し、部分値を返さない。
- read error と draw error が `RunError::Io` になり、cleanup が一度だけ実行される。
- finish error があっても cursor 表示と raw mode 無効化を試行する。
- reducer または validator の panic を `catch_unwind` した test で、guard の cleanup call 順を確認する。
- resize と狭い幅で state が変わらず、CJK の cursor 列と render 幅が再現可能である。
- 同じ `PromptView` と解決済み style bundle から同じ renderer command 列が得られる。

snapshot を使う場合も ANSI 全体だけを唯一の assertion にしない。
event sequence、field state、構造化 `PromptView`、terminal call log をそれぞれ検査し、失敗した境界を特定できるようにする。

## ratatui adapter との非重複

Prompt runtime と ratatui adapter は Theme の下流に並ぶ別 consumer である。
互いを呼び出さず、次の責務を共有しない。

| 責務 | Prompt runtime | ratatui adapter |
| --- | --- | --- |
| blocking event loop | 所有する | 所有しない |
| raw mode と cursor lifecycle | `Form::run` session が所有する | 所有しない |
| alternate screen | 切り替えない | adapter は所有せず、ratatui application が所有する |
| inline prompt の差分描画 | 所有する | 所有しない |
| `ratatui::Frame` と `Buffer` | 参照しない | 所有する |
| urushi `Style` から ratatui style への変換 | 行わない | 所有する |
| box model Widget | 所有しない | urushi の ratatui feature が所有する |
| Theme role と `TerminalProfile` 解決順 | Theme 契約を利用する | Theme 契約を利用する |

Prompt renderer を ratatui `Viewport::Inline` または `Buffer` の上に実装しない。
ratatui adapter に keyboard event、Form state、validation、terminal cleanup を追加しない。
両者が共有するのは urushi の `Theme`、`ComponentRole`、`Style`、`TerminalProfile` と CJK 幅規則だけである。

## 不変条件

- `Form::run` は blocking であり、async executor と外部 event loop を要求しない。
- submit、cancel、validation failure、I/O error は互いに異なる型または状態で表す。
- backend 固有 event 型は field、Form reducer、public API に現れない。
- 一つの event は active field に一度だけ適用し、一つの `FieldAction` を生む。
- validation に失敗した field から次の field へ進まない。
- Form の値は全 field の acceptance 後だけ公開する。
- Theme role は `TerminalProfile` で一度だけ解決し、Prompt 内で色値を再定義しない。
- renderer は inline 領域の外を消去せず、alternate screen を切り替えない。
- 取得済み terminal 資源の cleanup は終了経路ごとに最大一回だけ実行する。
- cleanup step の失敗は後続 cleanup step を省略する理由にしない。
- panic cleanup は元の panic を成功、cancel、validation、I/O errorへ変換しない。
- 同じ初期 Form、event 列、terminal dimensions、Theme、`TerminalProfile` は同じ状態列と `PromptView` 列を生む。

## 非目標

- 非同期 `run`、async validation、streaming validation。
- Elm runtime、command/subscription model、汎用 application runtime。
- alternate screen 前提の full-screen Prompt。
- mouse、paste event、動的 Form、動的 option loading、MultiSelect。
- process abort と強制 kill での terminal 復元保証。
- backend 差し替えを MVP の public extension API にすること。
- ratatui Widget または ratatui application event loop を Prompt crate に実装すること。

## 後続実装への割り当て

Form runtime の実装は公開結果型、Form と Group の reducer、terminal session guard、三つの deterministic seam を作る。
この段階で最小の test field を使い、normal、cancel、read error、render error、cleanup error、panic の call 順を検証する。

Input の実装は Unicode scalar 境界の編集、required と同期 validator、invalid state、CJK cursor 幅を追加する。
I/O と ANSI 描画は Input に追加しない。

Select と Confirm の実装は型付き option、循環 selection、empty option の構築エラー、`ConfirmSource` の区別を追加する。
独自 event loop と独自 option 配色は追加しない。

統合描画の実装は構造化 `PromptView`、狭い幅と resize、Theme role の style bundle、インライン再描画、公開 example を接続する。
疑似 backend の end-to-end test と実 terminal の手動 smoke 手順を用意し、合成 example を MVP evidence 自体とは扱わない。
