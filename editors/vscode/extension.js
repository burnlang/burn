"use strict";
const { execFile } = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");
const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");

let client;

function burnHome() {
  return process.env.BURN_HOME || path.join(os.homedir(), ".burn");
}

function burnPath() {
  const configured = vscode.workspace.getConfiguration("burn").get("path");
  if (configured && configured !== "burn") {
    return configured;
  }
  const installed = path.join(burnHome(), "bin", process.platform === "win32" ? "burn.exe" : "burn");
  return fs.existsSync(installed) ? installed : "burn";
}

function serverEnv() {
  const bin = path.join(burnHome(), "bin");
  const parts = (process.env.PATH || "").split(path.delimiter);
  const env = { ...process.env };
  env.PATH = parts.includes(bin) ? process.env.PATH : [bin, ...parts].join(path.delimiter);
  return env;
}

function startClient(context) {
  const command = burnPath();
  const folders = vscode.workspace.workspaceFolders;
  const options = { env: serverEnv(), cwd: folders && folders.length > 0 ? folders[0].uri.fsPath : undefined };
  const serverOptions = {
    run: { command, args: ["lsp"], options },
    debug: { command, args: ["lsp"], options },
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

function runBurn(args) {
  return new Promise((resolve, reject) => {
    execFile(burnPath(), args, { env: serverEnv() }, (err, stdout, stderr) => {
      if (err) {
        reject(new Error(stderr.trim() || err.message));
      } else {
        resolve(stdout.trim());
      }
    });
  });
}

function sourcesRoot() {
  return path.join(burnHome(), "cache", "sources");
}

function isLibrarySource(file) {
  const rel = path.relative(sourcesRoot(), file);
  return rel !== "" && !rel.startsWith("..") && !path.isAbsolute(rel);
}

async function openLibrarySource(pick) {
  try {
    const dir = await runBurn(["sources"]);
    const file = await pick(dir);
    if (file !== undefined) {
      const doc = await vscode.workspace.openTextDocument(file);
      await vscode.window.showTextDocument(doc, { preview: true });
    }
  } catch (err) {
    vscode.window.showErrorMessage(`Could not open the Burn sources: ${err.message}`);
  }
}

async function pickStdModule(dir) {
  const std = path.join(dir, "std");
  const items = fs
    .readdirSync(std)
    .filter((f) => f.endsWith(".bn"))
    .sort()
    .map((f) => ({ label: `std/${f.slice(0, -3)}`, file: path.join(std, f) }));
  const chosen = await vscode.window.showQuickPick(items, { placeHolder: "Standard library module to open" });
  return chosen ? chosen.file : undefined;
}

function runInTerminal(args, uri) {
  const open = async () => {
    if (uri) {
      return vscode.workspace.openTextDocument(uri);
    }
    const editor = vscode.window.activeTextEditor;
    return editor && editor.document.languageId === "burn" ? editor.document : undefined;
  };
  open().then(async (doc) => {
    if (!doc) {
      vscode.window.showWarningMessage("Open a .bn file first.");
      return;
    }
    await doc.save();
    const terminal = vscode.window.terminals.find((t) => t.name === "Burn") || vscode.window.createTerminal("Burn");
    terminal.show(true);
    terminal.sendText(`${JSON.stringify(burnPath())} ${args.join(" ")} ${JSON.stringify(doc.fileName)}`);
  });
}

const mainCodeLens = {
  provideCodeLenses(doc) {
    if (!vscode.workspace.getConfiguration("burn").get("codeLens.run", true) || isLibrarySource(doc.fileName)) {
      return [];
    }
    const lenses = [];
    for (let i = 0; i < doc.lineCount; i++) {
      if (/^\s*(async\s+)?fun\s+main\s*\(/.test(doc.lineAt(i).text)) {
        const range = doc.lineAt(i).range;
        lenses.push(
          new vscode.CodeLens(range, { title: "$(play) Run", command: "burn.run", arguments: [doc.uri] }),
          new vscode.CodeLens(range, { title: "Run natively", command: "burn.runNative", arguments: [doc.uri] }),
          new vscode.CodeLens(range, { title: "Build", command: "burn.build", arguments: [doc.uri] })
        );
      }
    }
    return lenses;
  },
};

function markReadOnly(editor) {
  if (editor && isLibrarySource(editor.document.fileName)) {
    vscode.commands.executeCommand("workbench.action.files.setActiveEditorReadonlyInSession");
  }
}

function createStatus(context) {
  const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  item.command = "burn.restartServer";
  item.text = "$(flame) Burn";
  item.tooltip = "Burn: restart the language server";
  runBurn(["version"]).then(
    (v) => {
      item.text = `$(flame) ${v}`;
      item.tooltip = `${v} (${burnPath()}): click to restart the language server`;
    },
    () => {
      item.text = "$(warning) Burn";
      item.tooltip = 'The burn executable was not found. Set "burn.path" or install Burn with burnup.';
    }
  );
  const update = (editor) => {
    if (editor && editor.document.languageId === "burn") {
      item.show();
    } else {
      item.hide();
    }
  };
  update(vscode.window.activeTextEditor);
  context.subscriptions.push(item, vscode.window.onDidChangeActiveTextEditor(update));
}

function activate(context) {
  startClient(context);
  createStatus(context);
  markReadOnly(vscode.window.activeTextEditor);
  context.subscriptions.push(
    vscode.commands.registerCommand("burn.run", (uri) => runInTerminal(["run"], uri)),
    vscode.commands.registerCommand("burn.runNative", (uri) => runInTerminal(["run", "--native"], uri)),
    vscode.commands.registerCommand("burn.build", (uri) => runInTerminal(["build"], uri)),
    vscode.commands.registerCommand("burn.openStdlib", () => openLibrarySource(pickStdModule)),
    vscode.commands.registerCommand("burn.openBuiltins", () => openLibrarySource((dir) => Promise.resolve(path.join(dir, "builtins.bn")))),
    vscode.languages.registerCodeLensProvider({ language: "burn" }, mainCodeLens),
    vscode.window.onDidChangeActiveTextEditor(markReadOnly),
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
