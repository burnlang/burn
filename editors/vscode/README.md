# Burn for Visual Studio Code

Language support for [Burn](https://github.com/burnlang/burn).

- Syntax highlighting for `.bn` files, including `def` definitions and string templates
- Live diagnostics from the real Burn compiler with exact line and column
- Hover with inferred types, signatures and documentation
- Completion for locals, globals, types, built-ins and members after `.`
- Signature help for calls and inlay hints with inferred types
- Go to definition, also into the standard library, built-in functions and bytecode libraries
- Find all references, highlight references and rename across files
- Workspace symbols, document outline, formatting and quick fixes
- Run, Run natively and Build links above `fun main`
- **Burn: Open Standard Library Module...** and **Burn: Show Built-in Functions** to read library sources

## Requirements

The extension talks to the language server built into the `burn` binary (`burn lsp`).
Install Burn and make sure `burn` is on your `PATH`, or set `burn.path` in the settings.

## Development

```sh
cd editors/vscode
npm install
npx vsce package
code --install-extension burn-language-server-26.2.0.vsix
```
