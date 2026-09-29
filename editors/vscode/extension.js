"use strict";
const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");

let client;

function burnPath() {
  return vscode.workspace.getConfiguration("burn").get("path") || "burn";
}

function startClient(context) {
  const command = burnPath();
  const serverOptions = {
    run: { command, args: ["lsp"] },
    debug: { command, args: ["lsp"] },
  };
  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "burn" }, { scheme: "untitled", language: "burn" }],
    synchronize: { fileEvents: vscode.workspace.createFileSystemWatcher("**/*.bn") },
  };
  client = new LanguageClient("burn", "Burn Language Server", serverOptions, clientOptions);
  client.start().catch((err) => {
    vscode.window.showErrorMessage(`Could not start the Burn language server (${command} lsp): ${err.message}. Set "burn.path" to your burn executable.`);
  });
  context.subscriptions.push({ dispose: () => client && client.stop() });
}

function runInTerminal(args) {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "burn") {
    vscode.window.showWarningMessage("Open a .bn file first.");
    return;
  }
  editor.document.save().then(() => {
    const terminal = vscode.window.terminals.find((t) => t.name === "Burn") || vscode.window.createTerminal("Burn");
    terminal.show(true);
    const file = JSON.stringify(editor.document.fileName);
    terminal.sendText(`${JSON.stringify(burnPath())} ${args.join(" ")} ${file}`);
  });
}

function activate(context) {
  startClient(context);
  context.subscriptions.push(
    vscode.commands.registerCommand("burn.run", () => runInTerminal(["run"])),
    vscode.commands.registerCommand("burn.runNative", () => runInTerminal(["run", "--native"])),
    vscode.commands.registerCommand("burn.build", () => runInTerminal(["build"])),
    vscode.commands.registerCommand("burn.restartServer", async () => {
      if (client) {
        await client.stop();
      }
      startClient(context);
      vscode.window.showInformationMessage("Burn language server restarted");
    })
  );
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
