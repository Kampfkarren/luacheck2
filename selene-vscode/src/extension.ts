import * as roblox from "./roblox"
import * as selene from "./selene"
import * as timers from "timers"
import * as util from "./util"
import * as vscode from "vscode"
import { Diagnostic, Severity, Label } from "./diagnostic"

let trySelene: Promise<boolean>

enum RunType {
    OnSave = "onSave",
    OnType = "onType",
    OnNewLine = "onNewLine",
    OnIdle = "onIdle",
}

function byteToCharMap(
    document: vscode.TextDocument,
    byteOffsets: Set<number>,
) {
    const text = document.getText()
    const byteOffsetMap = new Map<number, number>()
    let currentOffset = 0

    // Iterate through each character in the string
    for (let charOffset = 0; charOffset < text.length; charOffset++) {
        // Calculate the current byte offset we have reached so far
        currentOffset += Buffer.byteLength(text[charOffset], "utf-8")
        for (const offset of byteOffsets) {
            if (currentOffset >= offset) {
                byteOffsetMap.set(offset, charOffset + 1)
                byteOffsets.delete(offset)

                if (byteOffsets.size === 0) {
                    return byteOffsetMap
                }
            }
        }
    }

    return byteOffsetMap
}

function labelToRange(
    document: vscode.TextDocument,
    label: Label,
    byteOffsetMap: Map<number, number>,
): vscode.Range {
    return new vscode.Range(
        document.positionAt(
            byteOffsetMap.get(label.span.start) ?? label.span.start,
        ),
        document.positionAt(
            byteOffsetMap.get(label.span.end) ?? label.span.end,
        ),
    )
}

export async function activate(
    context: vscode.ExtensionContext,
): Promise<void> {
    console.log("selene-vscode activated")

    trySelene = util
        .ensureSeleneExists(context.globalStorageUri)
        .then(() => {
            return true
        })
        .catch((error) => {
            vscode.window.showErrorMessage(
                `An error occurred when finding selene:\n${error}`,
            )
            return false
        })

    await trySelene

    context.subscriptions.push(
        vscode.commands.registerCommand("selene.reinstall", () => {
            trySelene = util
                .downloadSelene(context.globalStorageUri)
                .then(() => true)
                .catch(() => false)
            return trySelene
        }),
    )

    const diagnosticsCollection =
        vscode.languages.createDiagnosticCollection("selene")
    context.subscriptions.push(diagnosticsCollection)

    let hasWarnedAboutRoblox = false

    async function lint(document: vscode.TextDocument) {
        if (document.languageId !== "lua") {
            return
        }

        if (!(await trySelene)) {
            return
        }

        const output = await selene.seleneCommand(
            context.globalStorageUri,
            "--display-style=json --no-summary -",
            selene.Expectation.Stderr,
            vscode.workspace.getWorkspaceFolder(document.uri),
            document.getText(),
        )

        if (!output) {
            diagnosticsCollection.delete(document.uri)
            return
        }

        const diagnostics: vscode.Diagnostic[] = []
        const dataToAdd: Diagnostic[] = []
        const byteOffsets = new Set<number>()

        for (const line of output.split("\n")) {
            const data: Diagnostic = JSON.parse(line)
            dataToAdd.push(data)
            byteOffsets.add(data.primary_label.span.start)
            byteOffsets.add(data.primary_label.span.end)
            for (const label of data.secondary_labels) {
                byteOffsets.add(label.span.start)
                byteOffsets.add(label.span.end)
            }
        }

        const byteOffsetMap = byteToCharMap(document, byteOffsets)

        for (const data of dataToAdd) {
            let message = data.message
            if (data.primary_label.message.length > 0) {
                message += `\n${data.primary_label.message}`
            }

            if (data.notes.length > 0) {
                message += `\n${data.notes.map((note) => `note: ${note}\n`)}`
            }

            const diagnostic = new vscode.Diagnostic(
                labelToRange(document, data.primary_label, byteOffsetMap),
                message,
                data.severity === Severity.Error
                    ? vscode.DiagnosticSeverity.Error
                    : vscode.DiagnosticSeverity.Warning,
            )

            diagnostic.source = `selene::${data.code}`

            if (data.code === "unused_variable") {
                diagnostic.tags = [vscode.DiagnosticTag.Unnecessary]
            }

            diagnostic.relatedInformation = data.secondary_labels.map(
                (label) => {
                    return {
                        message: label.message,
                        location: {
                            uri: document.uri,
                            range: labelToRange(document, label, byteOffsetMap),
                        },
                    }
                },
            )

            if (
                vscode.workspace
                    .getConfiguration("selene")
                    .get<boolean>("warnRoblox")
            ) {
                if (
                    !hasWarnedAboutRoblox &&
                    roblox.processDiagnostic(data, document)
                ) {
                    hasWarnedAboutRoblox = true
                }
            }

            diagnostics.push(diagnostic)
        }

        diagnosticsCollection.set(document.uri, diagnostics)
    }

    let lastTimeout: NodeJS.Timeout
    function listenToChange() {
        switch (
            vscode.workspace.getConfiguration("selene").get<RunType>("run")
        ) {
            case RunType.OnSave:
                return vscode.workspace.onDidSaveTextDocument(lint)
            case RunType.OnType:
                return vscode.workspace.onDidChangeTextDocument((event) =>
                    lint(event.document),
                )
            case RunType.OnNewLine:
                return vscode.workspace.onDidChangeTextDocument((event) => {
                    // Contrary to removing lines, adding new lines will leave the range at the same value hence the string comparisons
                    if (
                        event.contentChanges.some(
                            (content) =>
                                !content.range.isSingleLine ||
                                content.text === "\n" ||
                                content.text === "\r\n",
                        )
                    ) {
                        lint(event.document)
                    }
                })
            case RunType.OnIdle: {
                const idleDelay = vscode.workspace
                    .getConfiguration("selene")
                    .get<number>("idleDelay") as number

                return vscode.workspace.onDidChangeTextDocument((event) => {
                    timers.clearTimeout(lastTimeout)
                    lastTimeout = timers.setTimeout(
                        lint,
                        idleDelay,
                        event.document,
                    )
                })
            }
        }
    }

    let disposable = listenToChange()
    vscode.workspace.onDidChangeConfiguration((event) => {
        if (
            event.affectsConfiguration("selene.run") ||
            event.affectsConfiguration("selene.idleDelay")
        ) {
            disposable?.dispose()
            disposable = listenToChange()
        }
    })

    vscode.workspace.onDidOpenTextDocument(lint)
    vscode.workspace.onWillDeleteFiles((event) => {
        for (const documentUri of event.files) {
            diagnosticsCollection.set(documentUri, [])
        }
    })
    vscode.window.onDidChangeActiveTextEditor((editor) => {
        if (editor !== undefined) {
            lint(editor.document)
        }
    })
}

// this method is called when your extension is deactivated
export function deactivate(): void {
    return
}
