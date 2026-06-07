// Sparrow VS Code extension — thin wrapper that drives the existing
// WebView cockpit running locally and exposes Sparrow's CLI as commands.

const vscode = require("vscode");
const { spawn } = require("child_process");
const http = require("http");

let cockpitProcess = null;

function activate(context) {
    const config = () => vscode.workspace.getConfiguration("sparrow");

    const ensureCockpit = async () => {
        const port = config().get("cockpitPort", 9339);
        const reachable = await pingCockpit(port);
        if (reachable) return port;

        const bin = config().get("binaryPath", "sparrow");
        cockpitProcess = spawn(bin, ["console", "--port", String(port)], {
            detached: false,
            stdio: "ignore",
        });
        cockpitProcess.on("exit", () => {
            cockpitProcess = null;
        });

        // Wait up to 5 seconds for the cockpit to come online.
        for (let i = 0; i < 25; i++) {
            await new Promise((r) => setTimeout(r, 200));
            if (await pingCockpit(port)) return port;
        }
        throw new Error("Sparrow cockpit did not respond on port " + port);
    };

    context.subscriptions.push(
        vscode.commands.registerCommand("sparrow.openCockpit", async () => {
            try {
                const port = await ensureCockpit();
                const panel = vscode.window.createWebviewPanel(
                    "sparrowCockpit",
                    "Sparrow Cockpit",
                    vscode.ViewColumn.Beside,
                    { enableScripts: true, retainContextWhenHidden: true }
                );
                panel.webview.html = cockpitHtml(port);
            } catch (e) {
                vscode.window.showErrorMessage("Sparrow: " + e.message);
            }
        }),

        vscode.commands.registerCommand("sparrow.run", async () => {
            const task = await vscode.window.showInputBox({
                prompt: "Sparrow — task to run",
                placeHolder: "e.g. add a unit test for the login function",
            });
            if (!task) return;
            const term = vscode.window.createTerminal({ name: "Sparrow" });
            term.show();
            term.sendText(`${config().get("binaryPath", "sparrow")} run "${task.replace(/"/g, '\\"')}"`);
        }),

        vscode.commands.registerCommand("sparrow.plan", async () => {
            const task = await vscode.window.showInputBox({
                prompt: "Sparrow — task to plan (read-only)",
                placeHolder: "e.g. refactor the auth module",
            });
            if (!task) return;
            const term = vscode.window.createTerminal({ name: "Sparrow plan" });
            term.show();
            term.sendText(`${config().get("binaryPath", "sparrow")} plan "${task.replace(/"/g, '\\"')}"`);
        }),

        vscode.commands.registerCommand("sparrow.rewind", () => {
            const term = vscode.window.createTerminal({ name: "Sparrow rewind" });
            term.show();
            term.sendText(`${config().get("binaryPath", "sparrow")} rewind --last`);
        })
    );

    if (config().get("autoLaunchCockpit", false)) {
        ensureCockpit().catch(() => undefined);
    }
}

function pingCockpit(port) {
    return new Promise((resolve) => {
        const req = http.get(
            { host: "127.0.0.1", port, path: "/healthz", timeout: 250 },
            (res) => {
                resolve(res.statusCode === 200);
                res.resume();
            }
        );
        req.on("error", () => resolve(false));
        req.on("timeout", () => {
            req.destroy();
            resolve(false);
        });
    });
}

function cockpitHtml(port) {
    return `<!doctype html><html><head><meta charset="utf-8" />
<style>html,body,iframe{margin:0;padding:0;border:0;width:100%;height:100vh;}</style>
</head><body>
<iframe src="http://127.0.0.1:${port}/" allow="clipboard-read; clipboard-write"></iframe>
</body></html>`;
}

function deactivate() {
    if (cockpitProcess) {
        try {
            cockpitProcess.kill();
        } catch (_) {}
    }
}

module.exports = { activate, deactivate };
