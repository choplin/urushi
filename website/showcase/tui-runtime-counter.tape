Require cargo

Output "../public/docs/media/tui-runtime-counter.mp4"

Set Shell bash
Set FontFamily "Menlo"
Set FontSize 20
Set Width 840
Set Height 300
Set Theme "Catppuccin Mocha"
Set Padding 24
Set TypingSpeed 1ms
Set CursorBlink false

Hide
Type "env -u NO_COLOR TERM=xterm-256color COLORTERM=truecolor LC_ALL=en_US.UTF-8 cargo run --quiet --manifest-path terminal-fixtures/Cargo.toml --bin tui_runtime_counter"
Enter
Wait+Screen /Press any key to increment/
Show
Set TypingSpeed 45ms
Sleep 1s
Type "x"
Wait+Screen /key updates: 1/
Sleep 120ms
Type "x"
Wait+Screen /key updates: 2/
Wait+Screen /effect completions: 2/
Sleep 1s
Screenshot "../public/docs/media/tui-runtime-counter.png"
Type "q"
Wait+Screen /final count: 2/
Sleep 2s
