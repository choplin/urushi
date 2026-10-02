Require cargo

Output "../public/docs/media/prompt-wizard.mp4"

Set Shell bash
Set FontFamily "Menlo"
Set FontSize 18
Set Width 1100
Set Height 620
Set Theme "Catppuccin Mocha"
Set Padding 24
Set TypingSpeed 1ms
Set CursorBlink false

Hide
Type "env -u NO_COLOR TERM=xterm-256color COLORTERM=truecolor LC_ALL=en_US.UTF-8 cargo run --quiet --manifest-path terminal-fixtures/Cargo.toml --bin prompt_wizard"
Enter
Wait+Screen /What is your name?/
Show
Set TypingSpeed 45ms
Sleep 1s
Type "Aki"
Sleep 500ms
Enter
Wait+Screen /Choose a greeting style/
Sleep 1s
Down
Sleep 600ms
Enter
Wait+Screen /Generate the greeting?/
Sleep 1s
Screenshot "../public/docs/media/prompt-wizard.png"
Enter
Wait+Screen /Good day, Aki./
Sleep 2s
