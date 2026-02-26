package io.clawde

import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.actionSystem.CommonDataKeys
import com.intellij.openapi.ui.DialogWrapper
import com.intellij.openapi.ui.Messages
import kotlinx.serialization.json.*
import javax.swing.*
import java.awt.BorderLayout
import java.awt.Dimension

/**
 * Right-click → "Ask ClawDE…" action.
 *
 * Opens a dialog pre-filled with the selected code as context.
 * The user types a question; the response streams into the ClawDE tool window.
 *
 * Sprint KK JB.3
 */
class AskClawdAction : AnAction("Ask ClawDE…") {

    override fun update(e: AnActionEvent) {
        val editor = e.getData(CommonDataKeys.EDITOR)
        e.presentation.isEnabled = editor != null
    }

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val editor = e.getData(CommonDataKeys.EDITOR) ?: return
        val selection = editor.selectionModel.selectedText ?: ""
        val file = e.getData(CommonDataKeys.VIRTUAL_FILE)?.name ?: "file"

        val dialog = AskClawdDialog(project, selection, file)
        if (!dialog.showAndGet()) return

        val question = dialog.question.trim()
        if (question.isEmpty()) return

        val prompt = if (selection.isNotEmpty()) {
            "$question\n\n```\n// $file\n$selection\n```"
        } else {
            question
        }

        val client = ClawdClient()
        client.connect {
            client.call("session.create", buildJsonObject { put("provider", "claude") }) { result ->
                result.onSuccess { json ->
                    val sessionId = json.jsonObject["id"]?.jsonPrimitive?.content ?: return@onSuccess
                    client.call("session.send", buildJsonObject {
                        put("session_id", sessionId)
                        put("message", prompt)
                    }) { /* response streams via push events to tool window */ }
                }
                result.onFailure {
                    SwingUtilities.invokeLater {
                        Messages.showErrorDialog(project, "ClawDE: ${it.message}", "Connection Error")
                    }
                }
            }
        }
    }
}

class AskClawdDialog(
    project: com.intellij.openapi.project.Project,
    private val selection: String,
    private val filename: String,
) : DialogWrapper(project) {

    val questionField = JTextArea(4, 60).apply {
        lineWrap = true
        wrapStyleWord = true
    }
    val question: String get() = questionField.text

    init {
        title = "Ask ClawDE"
        init()
    }

    override fun createCenterPanel(): JComponent {
        val panel = JPanel(BorderLayout(0, 8))
        panel.preferredSize = Dimension(500, 200)

        if (selection.isNotEmpty()) {
            val preview = JLabel("<html><i>Context: ${selection.take(80).replace("<", "&lt;")}…</i></html>")
            preview.foreground = java.awt.Color.GRAY
            panel.add(preview, BorderLayout.NORTH)
        }

        panel.add(JLabel("Your question:"), BorderLayout.CENTER)
        panel.add(JScrollPane(questionField), BorderLayout.SOUTH)
        return panel
    }

    override fun getPreferredFocusedComponent() = questionField
}
