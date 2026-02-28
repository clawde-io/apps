package io.clawde

import com.intellij.openapi.Disposable
import com.intellij.openapi.diagnostic.logger
import kotlinx.serialization.json.*
import okhttp3.*
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicInteger

private val LOG = logger<ClawdClient>()

/**
 * Typed WebSocket/JSON-RPC 2.0 client for the clawd daemon.
 *
 * Connects to ws://127.0.0.1:4300 (default). Authenticates with a local
 * auth token read from `~/.claw/auth.token`. Each call is matched by
 * JSON-RPC `id` — callbacks are resolved on the OkHttp dispatcher thread.
 *
 * Sprint KK JB.2
 */
class ClawdClient(
    private val url: String = "ws://127.0.0.1:4300",
    private val authToken: String = readLocalAuthToken(),
) : Disposable {

    private val http = OkHttpClient()
    private var ws: WebSocket? = null
    private val idSeq = AtomicInteger(1)
    private val pending = ConcurrentHashMap<Int, (Result<JsonElement>) -> Unit>()
    private val pushListeners = mutableListOf<(method: String, params: JsonObject) -> Unit>()

    /** Connect and authenticate. Calls [onReady] when ready. */
    fun connect(onReady: () -> Unit = {}) {
        val request = Request.Builder().url(url).build()
        ws = http.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                call("daemon.auth", buildJsonObject { put("token", authToken) }) { result ->
                    result.fold(
                        onSuccess = { onReady() },
                        onFailure = { LOG.error("daemon.auth failed", it) },
                    )
                }
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                handleMessage(text)
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                LOG.warn("clawd WebSocket error: ${t.message}")
            }
        })
    }

    /** Send a JSON-RPC call and invoke [callback] with the result. */
    fun call(
        method: String,
        params: JsonObject = JsonObject(emptyMap()),
        callback: (Result<JsonElement>) -> Unit,
    ) {
        val id = idSeq.getAndIncrement()
        pending[id] = callback
        val msg = buildJsonObject {
            put("jsonrpc", "2.0")
            put("id", id)
            put("method", method)
            put("params", params)
        }
        ws?.send(msg.toString()) ?: callback(Result.failure(IllegalStateException("not connected")))
    }

    /** Register a listener for push notifications (method, params). */
    fun onPush(listener: (method: String, params: JsonObject) -> Unit) {
        pushListeners.add(listener)
    }

    private fun handleMessage(text: String) {
        val obj = Json.parseToJsonElement(text).jsonObject
        val id = obj["id"]?.jsonPrimitive?.intOrNull
        if (id != null) {
            val cb = pending.remove(id) ?: return
            val err = obj["error"]
            if (err != null) {
                cb(Result.failure(RuntimeException(err.jsonObject["message"]?.jsonPrimitive?.content ?: "RPC error")))
            } else {
                cb(Result.success(obj["result"] ?: JsonNull))
            }
        } else {
            // Push notification
            val method = obj["method"]?.jsonPrimitive?.content ?: return
            val params = obj["params"]?.jsonObject ?: JsonObject(emptyMap())
            pushListeners.forEach { it(method, params) }
        }
    }

    override fun dispose() {
        ws?.close(1000, "plugin disposed")
        http.dispatcher.executorService.shutdown()
    }
}

private fun readLocalAuthToken(): String {
    val home = System.getProperty("user.home") ?: return ""
    return try {
        java.io.File("$home/.claw/auth.token").readText().trim()
    } catch (_: Exception) {
        ""
    }
}
