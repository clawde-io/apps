package io.clawde

import com.intellij.openapi.project.Project
import com.intellij.openapi.wm.ToolWindow
import com.intellij.openapi.wm.ToolWindowFactory
import com.intellij.ui.content.ContentFactory
import com.intellij.ui.components.JBScrollPane
import com.intellij.ui.components.JBList
import kotlinx.serialization.json.*
import java.awt.BorderLayout
import java.awt.FlowLayout
import javax.swing.*

/**
 * Registers the ClawDE tool window (side panel) in JetBrains IDEs.
 *
 * Shows the list of active clawd sessions and a chat input field.
 * Sprint KK JB.1
 */
class ClawdToolWindowFactory : ToolWindowFactory {

    override fun createToolWindowContent(project: Project, toolWindow: ToolWindow) {
        val panel = ClawdPanel(project)
        val content = ContentFactory.getInstance().createContent(panel, "", false)
        toolWindow.contentManager.addContent(content)
    }
}

class ClawdPanel(private val project: Project) : JPanel(BorderLayout()) {
    private val client = ClawdClient()
    private val sessionModel = DefaultListModel<String>()
    private val sessionList = JBList(sessionModel)
    private val outputArea = JTextArea().apply {
        isEditable = false
        lineWrap = true
        wrapStyleWord = true
        background = UIManager.getColor("Panel.background")
    }
    private val inputField = JTextField()
    private val sendButton = JButton("Send")

    init {
        val topPanel = JPanel(BorderLayout())
        topPanel.add(JLabel("Sessions"), BorderLayout.NORTH)
        topPanel.add(JBScrollPane(sessionList).apply { preferredSize = java.awt.Dimension(0, 120) }, BorderLayout.CENTER)

        val centerPanel = JPanel(BorderLayout())
        centerPanel.add(JBScrollPane(outputArea), BorderLayout.CENTER)

        val inputPanel = JPanel(BorderLayout())
        inputPanel.add(inputField, BorderLayout.CENTER)
        inputPanel.add(sendButton, BorderLayout.EAST)

        add(topPanel, BorderLayout.NORTH)
        add(centerPanel, BorderLayout.CENTER)
        add(inputPanel, BorderLayout.SOUTH)

        sendButton.addActionListener { onSend() }
        inputField.addActionListener { onSend() }

        // Connect on creation
        client.connect { refreshSessions() }
        client.onPush { method, params ->
            if (method == "session.message.delta") {
                val text = params["delta"]?.jsonPrimitive?.content ?: ""
                SwingUtilities.invokeLater { outputArea.append(text) }
            }
        }
    }

    private fun refreshSessions() {
        client.call("session.list") { result ->
            result.onSuccess { json ->
                val sessions = json.jsonArray.map { it.jsonObject["title"]?.jsonPrimitive?.content ?: "Session" }
                SwingUtilities.invokeLater {
                    sessionModel.clear()
                    sessions.forEach { sessionModel.addElement(it) }
                }
            }
        }
    }

    private fun onSend() {
        val text = inputField.text.trim()
        if (text.isEmpty()) return
        inputField.text = ""
        outputArea.append("\n> $text\n")

        val selectedSession = sessionList.selectedValue
        if (selectedSession == null) {
            // Create a new session and send
            client.call("session.create", buildJsonObject { put("provider", "claude") }) { result ->
                result.onSuccess { json ->
                    val sessionId = json.jsonObject["id"]?.jsonPrimitive?.content ?: return@onSuccess
                    sendToSession(sessionId, text)
                }
            }
        } else {
            val idx = sessionList.selectedIndex
            client.call("session.list") { result ->
                result.onSuccess { json ->
                    val id = json.jsonArray.getOrNull(idx)?.jsonObject?.get("id")?.jsonPrimitive?.content ?: return@onSuccess
                    sendToSession(id, text)
                }
            }
        }
    }

    private fun sendToSession(sessionId: String, message: String) {
        client.call("session.send", buildJsonObject {
            put("session_id", sessionId)
            put("message", message)
        }) { /* streaming handled via push events */ }
    }
}
