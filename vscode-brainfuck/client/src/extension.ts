/* --------------------------------------------------------------------------------------------
 * Copyright (c) Microsoft Corporation. All rights reserved.
 * Licensed under the MIT License. See License.txt in the project root for license information.
 * ------------------------------------------------------------------------------------------ */

import {
	workspace,
	ExtensionContext,
	window,
	languages,
	debug,
	Terminal,
	DebugSession,
	TerminalOptions,
	commands,
} from "vscode";

import { platform } from 'os';
import { existsSync } from 'fs';

import {
	Executable,
	LanguageClient,
	LanguageClientOptions,
	RevealOutputChannelOn,
	ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient;
// type a = Parameters<>;

const lsp_program: Map<string, string> = new Map([["linux", "server/linux/brainfuck-lsp"], ["win32", "server/windows/brainfuck-lsp.exe"]]);
const interpreter_program: Map<string, string> = new Map([["linux", "server/linux/brainfuck-interpreter"], ["win32", "server/windows/brainfuck-interpreter.exe"]]);

export async function activate(context: ExtensionContext) {
	const traceOutputChannel = window.createOutputChannel("Brainfuck Language Server Client");

	const command = process.env.BRAINFUCK_LSPSERVER_PATH || context.asAbsolutePath(lsp_program.get(platform()));
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
	const config = workspace.getConfiguration("vscodeBrainfuck");
	const enableInlayHints = config.get("enableInlayHints", true);

	const clientOptions: LanguageClientOptions = {
		// Register the server for plain text documents
		documentSelector: [{ language: 'brainfuck' }],
		synchronize: {
			// Notify the server about file changes to '.clientrc files contained in the workspace
			fileEvents: workspace.createFileSystemWatcher("**/.clientrc"),
		},
		traceOutputChannel,
		revealOutputChannelOn: RevealOutputChannelOn.Info,
		initializationOptions: {
			enableInlayHints
		}
	};

	// Create the language client and start the client.
	client = new LanguageClient("vscodeBrainfuck", "Brainfuck Language Server", serverOptions, clientOptions);
	client.start();

	if (workspace.getConfiguration("vscodeBrainfuck").get("dapTrace") == "on") {
		const debugTraceOutputChannel = window.createOutputChannel("Brainfuck DAP Client");
		debug.registerDebugAdapterTrackerFactory('brainfuck', {
			createDebugAdapterTracker(session: DebugSession) {
				return {
					onWillReceiveMessage: m => debugTraceOutputChannel.appendLine(`> ${JSON.stringify(m, undefined, 2)}`),
					onDidSendMessage: m => debugTraceOutputChannel.appendLine(`< ${JSON.stringify(m, undefined, 2)}`)
				};
			}
		});
	}

	const interpreter = context.asAbsolutePath(interpreter_program.get(platform()));
	async function launch_interpreter(jit_config: string) {
		const file = window.activeTextEditor?.document.fileName;
		if (existsSync(file)) {
			const term = await createTerminal();
			term.show();
			term.sendText(interpreter + " --mode=" + jit_config + " --file=\"" + file + "\"");
		}
		else {
			window.showErrorMessage("Please open a valid .bf file.");
		}
	}
	context.subscriptions.push(
		commands.registerCommand("brainfuck.runWithJIT", () => launch_interpreter("jit")),
		commands.registerCommand("brainfuck.runAutoJIT", () => launch_interpreter("autojit")),
		commands.registerCommand("brainfuck.runWithoutJIT", () => launch_interpreter("interpret"))
	);
}



async function createTerminal(): Promise<Terminal> {
	const name = "Brainfuck/Launch";
	for (const term of window.terminals) {
		if (term.name == name) {
			return term;
		}
	}
	const options: TerminalOptions = {
		"name": name,
	};
	return window.createTerminal(options);
}

export function deactivate(): Thenable<void> | undefined {
	if (!client) {
		return undefined;
	}
	return client.stop();
}
