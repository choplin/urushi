# Theme の意味的語彙と Style 解決契約

## 契約の目的

この契約は、通常の CLI 出力、対話 Prompt、ratatui TUI が一つの Theme 定義から `urushi::Style` を取得する方法を固定する。

Theme は表示先を描画せず、端末能力を検出せず、Prompt の状態や ratatui の Widget を所有しない。
Theme が返すのは、意味とコンポーネントの意図を反映した、端末能力による縮退前の論理 `Style` である。

公開 API の名前と型の形は、この文書の Rust API sketch を M1 の実装契約とする。
M2 と M3 のアダプタは同じ解決順と role 名を使い、別の色表を導入しない。

## 所有境界

Theme を三つの層に分ける。
各層は一方向にだけ依存する。

1. **Semantic token**：色が表す意味を所有する。
   `text`、`accent`、`error` などの名前と、選択した配色における `Color` 値の対応である。
   modifier、padding、border、Prompt の状態、ratatui の型は所有しない。
2. **Component style**：semantic token を `Style` の foreground、background、modifier、box model に束ねる。
   `PromptOptionSelected` や `PanelFocused` のように、複数の表示先で再利用できる視覚部品を所有する。
   端末能力に応じた色変換と実行時状態は所有しない。
3. **Application extension**：urushi の共通語彙に含める根拠がないアプリ固有 token と component style を所有する。
   アプリは型付き extension を `Theme<E>` に格納し、型付き role で解決する。
   文字列キーの registry、実行時 theme discovery、グローバル可変 Theme は導入しない。

依存方向は `semantic token -> component style -> consumer adapter` である。
component style は token の値から構築し、通常出力、Prompt、ratatui の各アダプタで色値を再指定しない。

## Semantic token の最小語彙

MVP の `SemanticTokens` は次の十個を持つ。

| token | 意味 | 主な利用先 |
| --- | --- | --- |
| `text` | 通常の前景 | 本文、入力済み値、未選択 option |
| `text_muted` | 補助情報の前景 | help、placeholder、非活性表示 |
| `background` | 画面または出力領域の基底背景 | アプリが背景を明示して描画する領域 |
| `surface` | 基底背景から区別する面 | panel、Prompt のまとまり |
| `accent` | 現在位置、選択、主要操作 | cursor、選択 option、focused border |
| `accent_text` | `accent` を背景にしたときの前景 | 選択 option、強調ラベル |
| `success` | 成功した結果 | 完了メッセージ、成功状態 |
| `warning` | 続行可能な注意 | 警告メッセージ |
| `error` | 失敗または修正が必要な状態 | validation、失敗メッセージ |
| `border` | 通常の境界 | panel、区切り |

token 名は描画技術ではなく意味を表す。
たとえば `bright_red`、`ansi_212`、`input_blue` は semantic token 名にしない。

アプリ固有の意味を MVP の共通語彙へ追加する必要はない。
たとえば差分ビューの `addition` と `deletion` は application extension に置き、複数の独立アプリで同じ意味と利用規則が確認された後にだけ共通語彙への昇格を検討する。

## Component style の最小語彙

MVP の `ComponentRole` は次を持つ。

```rust
pub enum ComponentRole {
    Body,
    Muted,
    Accent,
    Success,
    Warning,
    Error,
    PromptQuestion,
    PromptAnswer,
    PromptPlaceholder,
    PromptCursor,
    PromptOption,
    PromptOptionSelected,
    PromptHelp,
    PromptError,
    Panel,
    PanelFocused,
}
```

`ComponentStyles::from_tokens` は次の初期対応を生成する。
利用者は `Theme::new` に別の `ComponentStyles` を渡して、この対応を配色ごとに置き換えられる。

| component role | token と style の初期対応 |
| --- | --- |
| `Body` | foreground `text` |
| `Muted` | foreground `text_muted`、dim |
| `Accent` | foreground `accent`、bold |
| `Success` | foreground `success` |
| `Warning` | foreground `warning` |
| `Error` | foreground `error`、bold |
| `PromptQuestion` | foreground `accent`、bold |
| `PromptAnswer` | foreground `text` |
| `PromptPlaceholder` | foreground `text_muted`、italic |
| `PromptCursor` | foreground `accent`、bold |
| `PromptOption` | foreground `text` |
| `PromptOptionSelected` | foreground `accent_text`、background `accent`、bold |
| `PromptHelp` | foreground `text_muted`、dim |
| `PromptError` | foreground `error` |
| `Panel` | foreground `text`、background `surface`、rounded border、border foreground `border`、左右一桁の padding |
| `PanelFocused` | `Panel` と同じ box model、border foreground `accent` |

`Panel` の本文 foreground と border foreground は別の `Style` property に入るため、同じ行の記述は競合しない。
box model を持つ component は plain renderer と ratatui Widget の双方へ渡せる。

Input は `PromptQuestion`、`PromptAnswer`、`PromptPlaceholder`、`PromptCursor`、`PromptHelp`、`PromptError` を使う。
Select はそれらに加えて `PromptOption` と `PromptOptionSelected` を使う。
Confirm は yes と no を option として扱い、Select と同じ二つの option role を使う。
field ごとの配色表は持たない。

## Light と dark の選択

一つの配色は `Theme<E>` で表し、light と dark の組は `ThemeSet<E>` で表す。
`ThemeSet::select` は `ColorScheme::Light` または `ColorScheme::Dark` を必ず受け取る。

`ColorScheme` に `Auto` variant と `Default` 実装は設けない。
呼び出し側は設定ファイル、CLI option、またはアプリ自身の既定値から light か dark を明示してから Theme を選ぶ。
端末背景色の推測は MVP に含めず、`TerminalProfile` も配色を選ばない。

light と dark は同じ extension 型 `E` と同じ role 集合を持つ。
この制約により、consumer は配色ごとに分岐せず `&Theme<E>` だけを受け取れる。

## 解決順序

Style の解決順序を次で固定する。

1. 呼び出し側が `ColorScheme` を決める。
2. `ThemeSet::select` が選択済みの `&Theme<E>` を返す。
3. `Theme::style(role)` が full-fidelity の論理 `&Style` を返す。
4. `TerminalProfile::resolve_style(&Style)` が出力先の色深度、`NO_COLOR`、TTY 方針に従う所有 `Style` を返す。
5. 解決済み `Style` を plain ANSI renderer、Prompt renderer、または ratatui adapter が消費する。

`TerminalProfile` は foreground、background、border color を決定的に縮退または除去する。
semantic role、modifier、padding、margin、border glyph、width、alignment は変更しない。
`NO_COLOR` と非 TTY の具体的な profile 選択規則は TerminalProfile の契約が所有し、Theme は環境変数や出力先を参照しない。

ratatui 変換は profile 解決後に行う。
先に ratatui の色へ変換すると、plain renderer と異なる縮退経路が生まれるためである。

```text
ColorScheme
    -> ThemeSet::select
    -> Theme::style
    -> TerminalProfile::resolve_style
    -> plain ANSI | Prompt renderer | ratatui adapter
```

## Rust API sketch

次の型と method を M1 の公開 API とする。
struct の field は private とし、`SemanticTokens` だけは構築のため public field を持つ。

```rust
use urushi::{Color, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticTokens {
    pub text: Color,
    pub text_muted: Color,
    pub background: Color,
    pub surface: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub border: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentRole {
    Body,
    Muted,
    Accent,
    Success,
    Warning,
    Error,
    PromptQuestion,
    PromptAnswer,
    PromptPlaceholder,
    PromptCursor,
    PromptOption,
    PromptOptionSelected,
    PromptHelp,
    PromptError,
    Panel,
    PanelFocused,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentStyles {
    // Private fields indexed by ComponentRole.
}

impl ComponentStyles {
    pub fn from_tokens(tokens: &SemanticTokens) -> Self;
    pub fn style(&self, role: ComponentRole) -> &Style;
    pub fn with_style(self, role: ComponentRole, style: Style) -> Self;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Theme<E = ()> {
    // Private fields: tokens, components, extension.
}

impl Theme<()> {
    pub fn from_tokens(tokens: SemanticTokens) -> Self;

    pub fn extend<E>(
        self,
        build: impl FnOnce(&SemanticTokens, &ComponentStyles) -> E,
    ) -> Theme<E>;
}

impl<E> Theme<E> {
    pub fn new(
        tokens: SemanticTokens,
        components: ComponentStyles,
        extension: E,
    ) -> Self;

    pub fn tokens(&self) -> &SemanticTokens;
    pub fn components(&self) -> &ComponentStyles;
    pub fn extension(&self) -> &E;

    pub fn style<R>(&self, role: R) -> &Style
    where
        R: ThemeRole<E>;
}

pub trait ThemeRole<E = ()>: Copy {
    fn resolve<'a>(self, theme: &'a Theme<E>) -> &'a Style;
}

impl<E> ThemeRole<E> for ComponentRole {
    fn resolve<'a>(self, theme: &'a Theme<E>) -> &'a Style;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSet<E = ()> {
    // Private fields: light, dark.
}

impl<E> ThemeSet<E> {
    pub fn new(light: Theme<E>, dark: Theme<E>) -> Self;
    pub fn light(&self) -> &Theme<E>;
    pub fn dark(&self) -> &Theme<E>;
    pub fn select(&self, scheme: ColorScheme) -> &Theme<E>;
}
```

`Theme::style` と `ComponentStyles::style` は `&Style` を返す。
表示先ごとに `Style` を clone する必要はなく、所有値が必要な profile 解決だけが新しい `Style` を返す。

`Theme::from_tokens` は `ComponentStyles::from_tokens` と `()` を組み合わせる便宜 constructor である。
`Theme::new` は共通 component を差し替える利用者向けであり、application extension を別 registry に逃がさない。

M1 の `TerminalProfile` は少なくとも次の境界を公開する。
profile の検出 constructor と `NO_COLOR`、非 TTY の既定規則は TerminalProfile 側で確定する。

```rust
impl TerminalProfile {
    pub fn resolve_style(&self, style: &Style) -> Style;
}
```

## Application extension

アプリ固有 Theme は `Theme<AppTheme>` として構築する。
extension はアプリ固有 token と component style を型でまとめる。

次の例では brand color を light と dark の各定義で一度だけ渡し、`ReportTitle` を三つの表示先から解決できるようにする。

```rust
use urushi::{Color, ComponentRole, ComponentStyles, SemanticTokens, Style, Theme, ThemeRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AppTokens {
    brand: Color,
}

#[derive(Debug, Clone, PartialEq)]
struct AppStyles {
    report_title: Style,
}

#[derive(Debug, Clone, PartialEq)]
struct AppTheme {
    tokens: AppTokens,
    styles: AppStyles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppRole {
    ReportTitle,
}

impl ThemeRole<AppTheme> for AppRole {
    fn resolve<'a>(self, theme: &'a Theme<AppTheme>) -> &'a Style {
        match self {
            Self::ReportTitle => &theme.extension().styles.report_title,
        }
    }
}

fn app_theme(tokens: SemanticTokens, brand: Color) -> Theme<AppTheme> {
    Theme::from_tokens(tokens).extend(move |_, components| {
        let app_tokens = AppTokens { brand };
        let report_title = components
            .style(ComponentRole::Body)
            .clone()
            .foreground(app_tokens.brand)
            .bold();

        AppTheme {
            tokens: app_tokens,
            styles: AppStyles { report_title },
        }
    })
}
```

アプリは `ThemeRole<AppTheme>` を実装するために urushi の enum を変更しない。
同じ型 `AppTheme` を light と dark の両 variant に使うため、consumer の型も変わらない。

## 三つの consumer での参照例

以下では `themes` が application の light と dark を一度ずつ定義済みで、`profile` は出力先について解決済みとする。
`theme` は全 consumer が共有する同じ参照である。

```rust
let theme = themes.select(ColorScheme::Dark);

let title = profile.resolve_style(theme.style(AppRole::ReportTitle));
println!("{}", title.render("Deployment report"));
```

Input、Select、Confirm は field 内で色を構築せず、必要な role を Theme から受け取る。
次の `InputStyles`、`SelectStyles`、`ConfirmStyles` は M3 の Prompt adapter が保持する解決済み style 束を表す。

```rust
let input_styles = InputStyles {
    question: profile.resolve_style(theme.style(ComponentRole::PromptQuestion)),
    answer: profile.resolve_style(theme.style(ComponentRole::PromptAnswer)),
    placeholder: profile.resolve_style(theme.style(ComponentRole::PromptPlaceholder)),
    cursor: profile.resolve_style(theme.style(ComponentRole::PromptCursor)),
    help: profile.resolve_style(theme.style(ComponentRole::PromptHelp)),
    error: profile.resolve_style(theme.style(ComponentRole::PromptError)),
};

let select_styles = SelectStyles {
    question: profile.resolve_style(theme.style(ComponentRole::PromptQuestion)),
    option: profile.resolve_style(theme.style(ComponentRole::PromptOption)),
    selected: profile.resolve_style(theme.style(ComponentRole::PromptOptionSelected)),
    help: profile.resolve_style(theme.style(ComponentRole::PromptHelp)),
    error: profile.resolve_style(theme.style(ComponentRole::PromptError)),
};

let confirm_styles = ConfirmStyles {
    question: profile.resolve_style(theme.style(ComponentRole::PromptQuestion)),
    option: profile.resolve_style(theme.style(ComponentRole::PromptOption)),
    selected: profile.resolve_style(theme.style(ComponentRole::PromptOptionSelected)),
    help: profile.resolve_style(theme.style(ComponentRole::PromptHelp)),
};
```

ratatui は同じ `AppRole::ReportTitle` と `ComponentRole::PanelFocused` を profile 解決してから変換する。
text modifier だけを使う箇所は ratatui の `Style` へ変換し、box model は urushi 所有 Widget へ渡す。

```rust
let title = profile.resolve_style(theme.style(AppRole::ReportTitle));
let title_style = ratatui::style::Style::from(&title);

let panel = profile.resolve_style(theme.style(ComponentRole::PanelFocused));
let widget = urushi::ratatui::StyledBlock::new("Deployment report").style(panel);
```

`InputStyles` などの束と `StyledBlock` の最終名は M3 と M2 で確定してよい。
ただし、それらが受け取る style、role、Theme の借用、profile 解決順はこの契約から変更しない。

この例では通常出力、Input、Select、Confirm、ratatui が `theme` を共有し、色値は light と dark の Theme 構築箇所以外に現れない。

## 不変条件

- `Theme::style` は同じ Theme と role に対して同じ論理 `Style` を返す。
- `Theme` と `ThemeSet` は構築後に不変であり、内部可変性とグローバル状態を持たない。
- `Theme` の構築と role 解決は terminal I/O、環境変数、ratatui、Prompt runtime に依存しない。
- `TerminalProfile` の適用は Theme 選択と role 解決の後、consumer adapter の前に一度だけ行う。
- Prompt と ratatui は semantic token の `Color` を直接参照せず、component role または application role から `Style` を得る。
- application extension は `Theme<E>` 内にあり、light と dark で同じ `E` を使う。
- profile 縮退後も可視文字、box model、CJK 幅、alignment は変わらない。

## 非目標

- 端末背景色の自動判定と `ColorScheme::Auto`。
- 実行時の theme discovery、TOML theme loader、Theme の hot reload。
- 文字列キーで任意 token や component を検索する registry。
- opaline など外部 theme engine へのコア依存。
  連携は将来の optional adapter または bridge crate が `SemanticTokens` と application extension を構築する形に限る。
- Theme に terminal capability、TTY、`NO_COLOR`、出力 stream を保持させること。
- Theme に Prompt の validation、focus 遷移、event loop、terminal lifecycle を保持させること。
- Theme に ratatui の `Frame`、`Buffer`、Widget 状態を保持させること。
- lipgloss または huh との完全な API 互換。
- MultiSelect、動的 option、async validation、高度な stateful ratatui Widget の role を先回りして追加すること。

## 後続実装への割り当て

M1 は `SemanticTokens`、`ComponentRole`、`ComponentStyles`、`Theme<E>`、`ThemeRole<E>`、`ThemeSet<E>`、`ColorScheme` と `TerminalProfile::resolve_style` を実装する。
M1 の plain CLI test は同じ Theme を light と dark で選び、profile 別に解決してから現行 `Style::render` へ渡す。

M2 は optional `ratatui` feature の内側で、解決済み `&Style` または `Style` から ratatui text style への変換と、box model を保持する urushi 所有 Widget を実装する。
M2 は Theme や semantic token に ratatui 型を追加しない。

M3 は Prompt renderer が必要とする role をこの契約の `ComponentRole` から取得する。
Input、Select、Confirm は Theme の所有権を要求せず、実行中に共有 `&Theme<E>` と `&TerminalProfile` から style 束を作れる境界にする。
