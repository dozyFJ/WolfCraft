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
import java.util.concurrent.TimeUnit

/**
 * Foreground service that owns the WolfServer process (the same server as on a PC, built for
 * Android and shipped as lib/arm64-v8a/libwolfserver.so).
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

        @Volatile var proc: Process? = null
        @Volatile var status: String = "Остановлен"
        @Volatile var world: String = ""
        /** Folder of the running world (a place), for the studio list. */
        @Volatile var worldId: String = ""
        /** Key for the server's control API (the dashboard calls it on 127.0.0.1). */
        @Volatile var token: String = ""
        private val lines = ArrayDeque<String>()

        fun running() = proc?.isAlive == true

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

    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(NOTIF_ID, notification(if (running()) status else "Запуск…"))
        if (intent?.action == ACTION_STOP) {
            Thread {
                stopServer()
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
            }.start()
            return START_NOT_STICKY
        }
        val name = intent?.getStringExtra(EXTRA_NAME)?.trim()?.ifEmpty { null } ?: "Сервер"
        val creative = intent?.getBooleanExtra(EXTRA_CREATIVE, false) ?: false
        val worldId = intent?.getStringExtra(EXTRA_WORLD)?.trim()?.ifEmpty { null }
        if (!running()) Thread { startServer(name, creative, worldId) }.start()
        return START_NOT_STICKY
    }

    private fun setStatus(s: String) {
        status = s
        getSystemService(NotificationManager::class.java).notify(NOTIF_ID, notification(s))
    }

    private fun startServer(name: String, creative: Boolean, worldId: String? = null) {
        val exe = File(applicationInfo.nativeLibraryDir, "libwolfserver.so")
        if (!exe.exists()) {
            setStatus("Ошибка: в приложении нет сервера (libwolfserver.so)")
            return
        }
        val data = File(filesDir, "wolfserver").apply { mkdirs() }
        token = java.util.UUID.randomUUID().toString().replace("-", "")
        val cmd = mutableListOf(exe.absolutePath, "--name", name, "--port", PORT.toString(), "--no-browser", "--data", data.absolutePath, "--token", token)
        if (creative) cmd += "--creative"
        if (worldId != null) { cmd += "--world"; cmd += worldId }
        try {
            if (wakeLock == null) {
                wakeLock = getSystemService(PowerManager::class.java)
                    .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "wolfserver:run").apply { acquire() }
            }
            if (wifiLock == null) {
                @Suppress("DEPRECATION")
                wifiLock = (applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager)
                    .createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "wolfserver:wifi").apply { acquire() }
            }
            addLog("▶ ${cmd.drop(1).dropLast(2).joinToString(" ")}")
            val pb = ProcessBuilder(cmd).directory(data).redirectErrorStream(true)
            pb.environment()["HOME"] = data.absolutePath
            pb.environment()["TMPDIR"] = cacheDir.absolutePath
            val p = pb.start()
            proc = p
            world = name
            Companion.worldId = worldId ?: ""
            setStatus("Работает: мир «$name», порт $PORT")
            // Server output into the log; when it ends, say how.
            Thread {
                try {
                    p.inputStream.bufferedReader().forEachLine { addLog(it) }
                } catch (_: Exception) {
                }
                val code = try { p.waitFor() } catch (_: Exception) { -1 }
                if (proc === p) {
                    proc = null
                    setStatus(if (code == 0 || code == 143) "Остановлен" else "Сервер завершился (код $code), смотри логи")
                    releaseLocks()
                }
            }.start()
        } catch (e: Exception) {
            addLog("error: $e")
            setStatus("Ошибка: ${e.message}")
            releaseLocks()
        }
    }

    private fun stopServer() {
        setStatus("Остановка…")
        val p = proc
        proc = null
        p?.let {
            it.destroy() // SIGTERM: the server saves and exits
            if (!it.waitFor(8, TimeUnit.SECONDS)) it.destroyForcibly()
        }
        addLog("■ остановлен")
        releaseLocks()
        status = "Остановлен"
    }

    private fun releaseLocks() {
        wakeLock?.let { if (it.isHeld) it.release() }
        wakeLock = null
        wifiLock?.let { if (it.isHeld) it.release() }
        wifiLock = null
    }

    override fun onDestroy() {
        releaseLocks()
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
