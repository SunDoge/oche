const vscode = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

let client;

async function activate(_context) {
  const command = vscode.workspace
    .getConfiguration('oche')
    .get('serverPath', 'oche');
  client = new LanguageClient(
    'oche',
    'Oche Language Server',
    {
      command,
      args: ['lsp'],
      transport: TransportKind.stdio,
    },
    {
      documentSelector: [{ scheme: 'file', language: 'oche' }],
    },
  );
  await client.start();
}

async function deactivate() {
  if (client) await client.stop();
}

module.exports = { activate, deactivate };
