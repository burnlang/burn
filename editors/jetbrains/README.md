# Burn for JetBrains IDEs

Language support for [Burn](https://github.com/burnlang/burn) in IntelliJ IDEA, CLion, RustRover, PyCharm and the
other JetBrains IDEs from 2024.2 on. Highlighting comes from the TextMate grammar in `../vscode`, everything else from
the Burn language server through [LSP4IJ](https://github.com/redhat-developer/lsp4ij): diagnostics, completion,
hover, parameter info, inlay hints, go to declaration (also into the standard library and built-ins), find usages,
rename, formatting and quick fixes. **Tools | Burn** runs, natively runs and builds the current file and opens the
library sources.

```sh
gradle buildPlugin
```

This needs JDK 21 and Gradle 9 or newer. Install `build/distributions/burn-intellij-26.2.0.zip` with **Settings |
Plugins | Install Plugin from Disk...**. The plugin starts `burn lsp` from your `PATH`, `$BURN_HOME/bin` or
`~/.burn/bin`.
