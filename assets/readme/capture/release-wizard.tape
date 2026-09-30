Require cargo
Output "release-wizard.gif"

Set Shell bash
Set FontFamily "Menlo"
Set FontSize 18
Set Width 1280
Set Height 720
Set Theme { "name": "Urushi TokyoNight", "black": "#15161e", "red": "#f7768e", "green": "#9ece6a", "yellow": "#e0af68", "blue": "#7aa2f7", "magenta": "#bb9af7", "cyan": "#7dcfff", "white": "#a9b1d6", "brightBlack": "#414868", "brightRed": "#f7768e", "brightGreen": "#9ece6a", "brightYellow": "#e0af68", "brightBlue": "#7aa2f7", "brightMagenta": "#bb9af7", "brightCyan": "#7dcfff", "brightWhite": "#c0caf5", "background": "#1a1b26", "foreground": "#c0caf5", "selection": "#33467c", "cursor": "#c0caf5" }
Set Padding 28
Set Framerate 20
Set TypingSpeed 35ms
Set CursorBlink false

Hide
Type "export TERM=xterm-256color COLORTERM=truecolor LC_ALL=en_US.UTF-8 PAGER=cat"
Enter
Type "unset NO_COLOR"
Enter
Type "export PS1='❯ '"
Enter
Type "cd ../.."
Enter
Type "clear"
Enter
Show

Type "cargo run --quiet -p urushi-prompt --example release_wizard"
Enter
Wait+Screen /Which version is ready?/
Sleep 800ms
Enter
Wait+Screen /Where should the release go first?/
Sleep 600ms
Type "/"
Type "mac"
Wait+Screen /macOS packages/
Sleep 700ms
Enter
Sleep 300ms
Enter
Wait+Screen /Create this release plan?/
Sleep 700ms
Type "y"
Wait+Screen /Release plan ready/
Sleep 2s
