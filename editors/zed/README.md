# Burn for Zed

Language support for [Burn](https://github.com/burnlang/burn) in [Zed](https://zed.dev): tree-sitter highlighting,
outline, indentation and brackets, plus the Burn language server (`burn lsp`) for diagnostics, completion, hover,
signature help, inlay hints, go to definition (also into the standard library and built-ins), references, rename and
formatting. A run button appears next to `fun main`.

Install it with **zed: install dev extension** and choose this folder. The extension starts `burn` from your `PATH`,
`$BURN_HOME/bin` or `~/.burn/bin`; set `lsp.burn.binary.path` in Zed's settings to use another one.
