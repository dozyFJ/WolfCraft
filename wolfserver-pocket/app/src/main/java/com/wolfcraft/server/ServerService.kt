package com.wolfcraft.server

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.graphics.drawable.Icon
import android.net.wifi.WifiManager
import android.os.IBinder
import android.os.PowerManager
import java.io.File
import com.wolfcraft.ota.OtaManager
import java.net.HttpURLConnection
import java.net.URL

/**
 * Foreground service hosting the signed, replaceable WolfServer JNI core.
 */
class ServerService : Service() {

    companion object {
        const val ACTION_START = "com.wolfcraft.server.START"
        const val ACTION_STOP = "com.wolfcraft.server.STOP"
        const val EXTRA_NAME = "name"
        const val EXTRA_CREATIVE = "creative"
        /** Folder of a WolfStudio place to run (its own world). */
        const val EXTRA_WORLD = "world"
        /** Game port (players connect here) and the local control panel. */
        const val PORT = 25570
        const val PANEL_PORT = 25580
        private const val CHANNEL = "server"
        private const val NOTIF_ID = 1

        @Volatile var coreThread: Thread? = null
        @Volatile private var coreLoaded = false
        @Volatile var status: String = "Остановлен"
        @Volatile var world: String = ""
        /** Folder of the running world (a place), for the studio list. */
        @Volatile var worldId: String = ""
        /** Key for the server's control API (the dashboard calls it on 127.0.0.1). */
        @Volatile var token: String = ""
        private val lines = ArrayDeque<String>()

        fun running() = coreThread?.isAlive == true

        fun addLog(line: String) {
            synchronized(lines) {
                lines.addLast(line)
                while (lines.size > 400) lines.removeFirst()
            }
        }

        fun logText(): String = synchronized(lines) { lines.joinToString("\n") }

        fun start(ctx: Context, name: String, creative: Boolean, worldId: String? = null) {
            ctx.startForegroundService(
                Intent(ctx, ServerService::class.java)
                    .setAction(ACTION_START)
                    .putExtra(EXTRA_NAME, name)
                    .putExtra(EXTRA_CREATIVE, creative)
                    .putExtra(EXTRA_WORLD, worldId)
            )
        }

        fun stop(ctx: Context) {
            ctx.startService(Intent(ctx, ServerService::class.java).setAction(ACTION_STOP))
        }
    }

    private val lifecycle = java.util.concurrent.Executors.newSingleThreadExecutor()

    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    @Volatile private var latestRequest = 0

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        latestRequest = startId
        OtaManager.registerPid(this, "main")
        startForeground(NOTIF_ID, notification(if (running()) status else "Запуск…"))
        if (intent?.action == ACTION_STOP) {
            lifecycle.execute {
                val stopped = stopServer()
                if (stopped && latestRequest == startId) {
                    stopForeground(STOP_FOREGROUND_REMOVE)
                    stopSelf(startId)
                }
            }
            return START_NOT_STICKY
        }
        val name = intent?.getStringExtra(EXTRA_NAME)?.trim()?.ifEmpty { null } ?: "Сервер"
        val creative = intent?.getBooleanExtra(EXTRA_CREATIVE, false) ?: false
        val worldId = intent?.getStringExtra(EXTRA_WORLD)?.trim()?.ifEmpty { null }
        lifecycle.execute { if (!running()) startServer(name, creative, worldId) }
        return START_NOT_STICKY
    }

    private fun setStatus(s: String) {
        status = s
        getSystemService(NotificationManager::class.java).notify(NOTIF_ID, notification(s))
    }

    private external fun nativeRun(args: Array<String>): String?
    private external fun nativeStop()

    private fun startServer(name: String, creative: Boolean, worldId: String? = null) {
        val data = File(filesDir, "wolfserver").apply { mkdirs() }
        token = java.util.UUID.randomUUID().toString().replace("-", "")
        OtaManager.prefs(this).edit().putString("server_token", token).apply()
        val args = mutableListOf("--name", name, "--port", PORT.toString(), "--no-browser", "--data", data.absolutePath, "--token", token)
        if (worldId == null) args += "--planet"
        args += if (creative) "--creative" else "--survival"
        if (worldId != null) { args += "--world"; args += worldId }
        try {
            if (!coreLoaded) {
                OtaManager.loadServer(this)
                coreLoaded = true
            }
            if (wakeLock == null) wakeLock = getSystemService(PowerManager::class.java)
                .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "wolfserver:run").apply { acquire() }
            if (wifiLock == null) {
                @Suppress("DEPRECATION")
                wifiLock = (applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager)
                    .createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "wolfserver:wifi").apply { acquire() }
            }
            addLog("▶ WolfServer · $name · ${if (worldId == null) "обычный мир" else "мир $worldId"} · порт $PORT")
            world = name
            Companion.worldId = worldId ?: ""
            setStatus("Запуск мира «$name»…")
            val thread = Thread({
                val error = try { nativeRun(args.toTypedArray()).also { if (it == null || !it.startsWith("Ошибка ядра")) OtaManager.markHealthy(this, "studio", "server") } } catch (e: Throwable) { e.message ?: "Ошибка ядра" }
                if (coreThread === Thread.currentThread()) {
                    coreThread = null
                    if (error != null) addLog("Ошибка ядра: $error") else addLog("■ остановлен")
                    setStatus(if (error == null) "Остановлен" else "Ошибка: $error")
                    releaseLocks()
                }
            }, "wolfserver-core")
            coreThread = thread
            thread.start()
            Thread({
                repeat(60) {
                    if (!thread.isAlive) return@Thread
                    try {
                        val c = URL("http://127.0.0.1:$PANEL_PORT/api/status?t=$token").openConnection() as HttpURLConnection
                        c.connectTimeout = 500; c.readTimeout = 500
                        val ok = c.responseCode == 200
                        c.inputStream.close(); c.disconnect()
                        if (ok && thread.isAlive && coreThread === thread) {
                            OtaManager.markHealthy(this, "studio", "server")
                            setStatus("Работает: мир «$name», порт $PORT")
                            return@Thread
                        }
                    } catch (_: Exception) { }
                    Thread.sleep(250)
                }
            }, "wolfserver-ready").start()
        } catch (e: Throwable) {
            addLog("Ошибка загрузки ядра: ${e.message}")
            setStatus("Ядро не загружено. Перезапусти приложение.")
            OtaManager.rollback(this, "studio", "Ошибка загрузки сервера: ${e.javaClass.simpleName}")
            releaseLocks()
        }
    }

    private fun stopServer(): Boolean {
        setStatus("Остановка…")
        val thread = coreThread
        if (thread != null && coreLoaded) {
            try { nativeStop() } catch (e: Throwable) { addLog("Остановка ядра: ${e.message}") }
            thread.join(8000)
            if (thread.isAlive) { setStatus("Сервер ещё сохраняется…"); return false }
        }
        releaseLocks()
        status = "Остановлен"
        return true
    }

    private fun releaseLocks() {
        wakeLock?.let { if (it.isHeld) it.release() }
        wakeLock = null
        wifiLock?.let { if (it.isHeld) it.release() }
        wifiLock = null
    }

    override fun onDestroy() {
        lifecycle.shutdown()
        if (running() && coreLoaded) Thread { try { nativeStop() } catch (_: Throwable) { } }.start()
        else releaseLocks()
        super.onDestroy()
    }

    private fun notification(text: String): Notification {
        val nm = getSystemService(NotificationManager::class.java)
        if (nm.getNotificationChannel(CHANNEL) == null) {
            nm.createNotificationChannel(NotificationChannel(CHANNEL, "Сервер", NotificationManager.IMPORTANCE_LOW))
        }
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        val stop = PendingIntent.getService(
            this, 1, Intent(this, ServerService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE
        )
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_stat)
            .setContentTitle("WolfStudio · сервер")
            .setContentText(text.lineSequence().first())
            .setOngoing(true)
            .setContentIntent(open)
            .addAction(Notification.Action.Builder(Icon.createWithResource(this, R.drawable.ic_stat), "Стоп", stop).build())
            .build()
    }
}


