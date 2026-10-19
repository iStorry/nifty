import { commands, type ExtensionContext, workspace } from "vscode";
import {
  LanguageClient,
  type LanguageClientOptions,
  type ServerOptions,
} from "vscode-languageclient/node";

const LANGUAGE_ID = "nifty";
const DEFAULT_SERVER_PATH = "nf";

let client: LanguageClient | undefined;

export async function activate(context: ExtensionContext): Promise<void> {
  const command = workspace
    .getConfiguration(LANGUAGE_ID)
    .get<string>("server.path", DEFAULT_SERVER_PATH);
  const serverOptions: ServerOptions = { command, args: ["lsp"] };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: LANGUAGE_ID },
      { scheme: "untitled", language: LANGUAGE_ID },
    ],
  };
  const languageClient = new LanguageClient(LANGUAGE_ID, "Nifty", serverOptions, clientOptions);
  client = languageClient;

  context.subscriptions.push(
    commands.registerCommand("nifty.restartServer", () => languageClient.restart()),
  );
  await languageClient.start();
}

export function deactivate(): Promise<void> | undefined {
  return client?.stop();
}
