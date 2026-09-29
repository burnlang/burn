# Burn for Visual Studio Code

Language support for [Burn](https://github.com/burnlang/burn).

- Syntax highlighting for `.bn` files, including `def` definitions and string templates
- Live diagnostics from the real Burn compiler with exact line and column
- Hover with inferred types and signatures
- Completion for locals, globals, types, built-ins and members after `.`
- Go to definition, document outline and formatting
- Commands to run, natively compile and build the current file

## Requirements

The extension talks to the language server built into the `burn` binary (`burn lsp`).
Install Burn and make sure `burn` is on your `PATH`, or set `burn.path` in the settings.

## Development

```sh
cd editors/vscode
npm install
npx vsce package
code --install-extension burn-language-server-26.1.0.vsix
```
