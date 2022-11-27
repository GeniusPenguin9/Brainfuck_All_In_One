/* --------------------------------------------------------------------------------------------
 * Copyright (c) Microsoft Corporation. All rights reserved.
 * Licensed under the MIT License. See License.txt in the project root for license information.
 * ------------------------------------------------------------------------------------------ */

import {
	workspace,
	ExtensionContext,
	window,
} from "vscode";

import {
	Executable,
	LanguageClient,
	LanguageClientOptions,
	RevealOutputChannelOn,
	ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient;
// type a = Parameters<>;

export async function activate(context: ExtensionContext) {
	const traceOutputChannel = window.createOutputChannel("Brainfuck Language Server");
	const command = process.env.SERVER_PATH || "C:\\Users\\cauli\\source\\repos\\rust\\brainfuck-lsp\\brainfuck-lsp\\target\\debug\\brainfuck-lsp.exe";
	traceOutputChannel.appendLine("starting command: " + command);
	const run: Executable = {
		command,
		options: {
			env: {
				...process.env,
			},
		},
	};
	const serverOptions: ServerOptions = {
		run,
		debug: run,
	};
	// If the extension is launched in debug mode then the debug server options are used
	// Otherwise the run options are used
	// Options to control the language client
	const clientOptions: LanguageClientOptions = {
		// Register the server for plain text documents
		documentSelector: [{ pattern: "*.bf" }],
		synchronize: {
			// Notify the server about file changes to '.clientrc files contained in the workspace
			fileEvents: workspace.createFileSystemWatcher("**/.clientrc"),
		},
		traceOutputChannel,
		revealOutputChannelOn: RevealOutputChannelOn.Info
	};

	// Create the language client and start the client.
	client = new LanguageClient("vscodeBrainfuck", "Brainfuck Language Server", serverOptions, clientOptions);
	client.start();
}

export function deactivate(): Thenable<void> | undefined {
	if (!client) {
		return undefined;
	}
	return client.stop();
}
