import * as path from 'path';
import { workspace, ExtensionContext, window } from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  Executable
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
  // We assume the pace compiler provides an `lsp` command
  // E.g. `pace lsp`
  
  const runExecutable: Executable = {
    command: 'pace', // This relies on `pace` being in the system PATH
    args: ['lsp']
  };

  const serverOptions: ServerOptions = {
    run: runExecutable,
    debug: runExecutable
  };

  const clientOptions: LanguageClientOptions = {
    // Register the server for pace documents
    documentSelector: [{ scheme: 'file', language: 'pace' }],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher('**/*.pace')
    }
  };

  client = new LanguageClient(
    'paceLanguageServer',
    'Pace Language Server',
    serverOptions,
    clientOptions
  );

  // Start the client. This will also launch the server
  client.start().catch((err: any) => {
    console.log("Language server could not be started. Note: The Pace compiler must have an 'lsp' subcommand implemented and 'pace' must be in your PATH.", err);
  });
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
