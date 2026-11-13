package com.wolfcraft.server

import android.app.Activity
import android.app.AlertDialog
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.provider.Settings
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.Inet4Address
import java.net.NetworkInterface
import java.net.URL
import java.net.URLEncoder

/**
 * WolfServer Pocket: a dashboard of the server running on this phone, with a bottom bar —
 * Главная (dashboard), Сервер (start, world, players), Настройки, Консоль (log + messages).
 * Data comes from the server's own API on 127.0.0.1 (the same one its browser panel uses).
 */
class MainActivity : Activity() {

    private val handler = Handler(Looper.getMainLooper())

    // Material dark with a blue accent (matches the game's new look).
    private val cBg = Color.rgb(14, 16, 20)
    private val cCard = Color.rgb(23, 26, 32)
    private val cCardHi = Color.rgb(31, 35, 43)
    private val cLine = Color.rgb(45, 51, 62)
    private val cText = Color.rgb(230, 233, 239)
    private val cMuted = Color.rgb(154, 163, 178)
    private val cAccent = Color.rgb(37, 99, 235)
    private val cGreen = Color.rgb(61, 214, 140)
    private val cRed = Color.rgb(240, 84, 91)

    /** Last status from the server API (null while stopped or not answering yet). */
    @Volatile private var status: JSONObject? = null
    @Volatile private var polling = false

    private var tab = 0
    private lateinit var content: LinearLayout
    private lateinit var scroll: ScrollView
    private val navButtons = mutableListOf<TextView>()
    private var lastRender = ""

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()

    private fun rounded(color: Int, radius: Int = 16, stroke: Int? = null) = GradientDrawable().apply {
        setColor(color)
        cornerRadius = dp(radius).toFloat()
        if (stroke != null) setStroke(dp(1), stroke)
    }

    private fun label(t: String, size: Float = 15f, color: Int = cText, bold: Boolean = false) = TextView(this).apply {
        this.text = t
        textSize = size
        setTextColor(color)
        if (bold) typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
    }

    private fun caption(t: String) = label(t.uppercase(), 12f, cMuted, true).apply { letterSpacing = 0.08f; setPadding(0, 0, 0, dp(8)) }

    private fun button(t: String, kind: Int = 0, onClick: () -> Unit) = Button(this).apply {
        // kind: 0 tonal, 1 primary (blue), 2 danger
        this.text = t
        isAllCaps = false
        textSize = 15f
        stateListAnimator = null
        setTextColor(if (kind == 0) cText else Color.WHITE)
        background = rounded(when (kind) { 1 -> cAccent; 2 -> cRed; else -> cCardHi }, 26)
        setOnClickListener { onClick() }
        layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(50)).apply { topMargin = dp(10) }
    }

    private fun card(build: LinearLayout.() -> Unit) = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        background = rounded(cCard, 18, cLine)
        setPadding(dp(18), dp(16), dp(18), dp(16))
        layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply { topMargin = dp(12) }
        build()
    }

    private fun row(key: String, value: String) = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        setPadding(0, dp(10), 0, dp(10))
        addView(label(key, 15f, cMuted).apply { layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f) })
        addView(label(value, 15f, cText).apply { gravity = Gravity.END })
    }

    private fun divider() = View(this).apply {
        setBackgroundColor(cLine)
        layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(1))
    }

    private fun input(hintText: String, value: String = "") = EditText(this).apply {
        setText(value)
        hint = hintText
        setTextColor(cText)
        setHintTextColor(cMuted)
        textSize = 15f
        background = rounded(cBg, 14, cLine)
        setPadding(dp(14), dp(12), dp(14), dp(12))
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.statusBarColor = cBg
        window.navigationBarColor = cCard
        tab = savedInstanceState?.getInt("tab") ?: 0

        val root = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setBackgroundColor(cBg) }
        content = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(dp(16), dp(18), dp(16), dp(24)) }
        scroll = ScrollView(this).apply {
            addView(content)
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f)
        }
        root.addView(scroll)

        // Bottom navigation: Главная · Сервер · Настройки · Консоль.
        val nav = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setBackgroundColor(cCard)
            setPadding(dp(8), dp(8), dp(8), dp(10))
        }
        val items = listOf("⌂" to "Главная", "▶" to "Сервер", "⚙" to "Настройки", "›_" to "Консоль")
        items.forEachIndexed { i, (icon, name) ->
            val b = TextView(this).apply {
                this.text = "$icon\n$name"
                gravity = Gravity.CENTER
                textSize = 12.5f
                setPadding(0, dp(6), 0, dp(6))
                layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f).apply { setMargins(dp(4), 0, dp(4), 0) }
                setOnClickListener { selectTab(i) }
            }
            navButtons += b
            nav.addView(b)
        }
        root.addView(View(this).apply { setBackgroundColor(cLine); layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(1)) })
        root.addView(nav)
        setContentView(root)
        selectTab(tab)

        if (android.os.Build.VERSION.SDK_INT >= 33) {
            requestPermissions(arrayOf("android.permission.POST_NOTIFICATIONS"), 1)
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putInt("tab", tab)
    }

    override fun onResume() {
        super.onResume()
        polling = true
        Thread { pollLoop() }.start()
        handler.post(tick)
    }

    override fun onPause() {
        polling = false
        handler.removeCallbacks(tick)
        super.onPause()
    }

    private fun renderKey() = "$tab|${ServerService.running()}|${ServerService.status}|${status?.toString()}|${ServerService.logText().length}"

    private val tick = object : Runnable {
        override fun run() {
            // Rebuild only when something shown changed, and never while the user is typing.
            if (renderKey() != lastRender && currentFocus !is EditText) render()
            handler.postDelayed(this, 1000)
        }
    }

    private fun selectTab(i: Int) {
        tab = i
        navButtons.forEachIndexed { j, b ->
            val on = j == i
            b.setTextColor(if (on) Color.WHITE else cMuted)
            b.background = if (on) rounded(Color.argb(70, 37, 99, 235), 16) else null
        }
        currentFocus?.clearFocus()
        render()
        scroll.scrollTo(0, 0)
    }

    private fun render() {
        lastRender = renderKey()
        content.removeAllViews()
        when (tab) {
            0 -> renderHome()
            1 -> renderServer()
            2 -> renderSettings()
            else -> renderConsole()
        }
    }

    // ---- Data -------------------------------------------------------------------------------

    private fun pollLoop() {
        while (polling) {
            status = if (ServerService.running() && ServerService.token.isNotEmpty()) {
                api("GET", "/api/status")?.let { runCatching { JSONObject(it) }.getOrNull() }
            } else {
                null
            }
            Thread.sleep(1000)
        }
    }

    /** Calls the server API on this phone; null when it does not answer. */
    private fun api(method: String, path: String, query: String = ""): String? = try {
        val c = URL("http://127.0.0.1:${ServerService.PANEL_PORT}$path?t=${ServerService.token}$query").openConnection() as HttpURLConnection
        c.requestMethod = method
        c.connectTimeout = 1500
        c.readTimeout = 2500
        val body = c.inputStream.bufferedReader().readText()
        c.disconnect()
        body
    } catch (_: Exception) {
        null
    }

    private fun apiAsync(method: String, path: String, query: String = "") {
        Thread { api(method, path, query) }.start()
    }

    private fun lanIps(): List<String> = try {
        NetworkInterface.getNetworkInterfaces().toList()
            .filter { it.isUp && !it.isLoopback }
            .flatMap { it.inetAddresses.toList() }
            .filterIsInstance<Inet4Address>()
            .mapNotNull { it.hostAddress }
    } catch (_: Exception) {
        emptyList()
    }

    private fun addresses(): List<String> {
        val fromServer = status?.optJSONArray("addresses")?.let { a -> (0 until a.length()).map { a.getString(it) } }.orEmpty()
        return fromServer.ifEmpty { lanIps().map { "$it:${ServerService.PORT}" } }
    }

    private fun uptime(sec: Long): String = if (sec < 3600) "${sec / 60} мин ${sec % 60} с" else "${sec / 3600} ч ${sec % 3600 / 60} мин"

    // ---- Tabs -------------------------------------------------------------------------------

    private fun header(title: String) {
        val on = ServerService.running()
        val bar = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }
        bar.addView(LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            addView(label(title, 24f, cText, true))
            val ver = status?.optString("version")?.takeIf { it.isNotEmpty() }
            addView(label("WolfServer Pocket" + (ver?.let { " · v$it" } ?: ""), 12.5f, cMuted))
        })
        bar.addView(label(if (on) "РАБОТАЕТ" else "ОСТАНОВЛЕН", 12f, if (on) cGreen else cMuted, true).apply {
            letterSpacing = 0.08f
            background = rounded(Color.TRANSPARENT, 10, if (on) cGreen else cLine)
            setPadding(dp(12), dp(6), dp(12), dp(6))
        })
        content.addView(bar)
    }

    private fun renderHome() {
        header("Главная")
        val s = status
        val on = ServerService.running()
        content.addView(card {
            addView(caption("Адрес для подключения"))
            if (on) {
                val list = addresses()
                if (list.isEmpty()) addView(label("Нет Wi‑Fi: включи Wi‑Fi или точку доступа", 15f, cMuted))
                list.forEach { addView(label(it, 26f, cAccent, true).apply { setTextIsSelectable(true) }) }
                addView(label("На этом телефоне: 127.0.0.1:${ServerService.PORT}", 13.5f, cMuted).apply { setPadding(0, dp(6), 0, 0) })
                addView(label("В игре: Играть → Игра по сети → «Рядом» или «По коду».", 13.5f, cMuted))
                addView(button("Скопировать адрес") { copyAddress() })
            } else {
                addView(label("Сервер выключен", 18f, cMuted, true))
                addView(button("▶  Запустить", 1) { startServer() })
            }
        })
        if (on) {
            content.addView(card {
                addView(caption("Мир"))
                val rows = listOf(
                    "Название" to (s?.optString("name") ?: ServerService.world),
                    "Seed" to (s?.optString("seed") ?: "…"),
                    "Режим" to (s?.optString("mode") ?: "…"),
                    "Изменено блоков" to (s?.optInt("edits")?.toString() ?: "…"),
                    "Работает" to (s?.optLong("uptime")?.let { uptime(it) } ?: "…"),
                )
                rows.forEachIndexed { i, (k, v) ->
                    addView(row(k, v))
                    if (i < rows.size - 1) addView(divider())
                }
            })
            playersCard()
        }
    }

    private fun playersCard() {
        val players = status?.optJSONArray("players")
        val n = players?.length() ?: 0
        content.addView(card {
            addView(caption("Игроки ($n)"))
            if (n == 0) addView(label("Никого нет", 15f, cMuted))
            for (i in 0 until n) {
                val p = players!!.getJSONObject(i)
                val r = LinearLayout(this@MainActivity).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL; setPadding(0, dp(6), 0, dp(6)) }
                r.addView(label((if (p.optBoolean("hell")) "🔥 " else "● ") + p.optString("nick"), 16f, cText).apply { layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f) })
                r.addView(Button(this@MainActivity).apply {
                    this.text = "Выгнать"
                    isAllCaps = false
                    setTextColor(cRed)
                    stateListAnimator = null
                    background = rounded(Color.TRANSPARENT, 20, cRed)
                    setOnClickListener { apiAsync("POST", "/api/kick", "&id=${p.optInt("id")}") }
                })
                addView(r)
            }
        })
    }

    private fun renderServer() {
        header("Сервер")
        val on = ServerService.running()
        val prefs = getSharedPreferences("ui", MODE_PRIVATE)
        content.addView(card {
            addView(caption("Мир"))
            val name = input("Название мира", prefs.getString("name", "Сервер") ?: "Сервер").apply { isEnabled = !on }
            addView(name)
            val creative = CheckBox(this@MainActivity).apply {
                this.text = "Творческий режим (для нового мира)"
                setTextColor(cText)
                isChecked = prefs.getBoolean("creative", false)
                isEnabled = !on
            }
            addView(creative)
            if (on) {
                addView(button("■  Остановить", 2) { ServerService.stop(this@MainActivity) })
            } else {
                addView(button("▶  Запустить", 1) {
                    prefs.edit().putString("name", name.text.toString()).putBoolean("creative", creative.isChecked).apply()
                    name.clearFocus()
                    ServerService.start(this@MainActivity, name.text.toString(), creative.isChecked)
                })
            }
            addView(label(ServerService.status, 13.5f, cMuted).apply { setPadding(0, dp(10), 0, 0) })
        })
        if (on) {
            content.addView(sayCard("Сообщение всем игрокам"))
            playersCard()
        }
    }

    private fun sayCard(title: String) = card {
        addView(caption(title))
        val msg = input("Текст в чат игры")
        addView(msg)
        addView(button("Отправить", 1) {
            val t = msg.text.toString().trim()
            if (t.isNotEmpty()) {
                apiAsync("POST", "/api/say", "&text=" + URLEncoder.encode(t, "UTF-8"))
                msg.setText("")
                msg.clearFocus()
                Toast.makeText(this@MainActivity, "Отправлено", Toast.LENGTH_SHORT).show()
            }
        })
    }

    private fun renderSettings() {
        header("Настройки")
        content.addView(card {
            addView(caption("Работа в фоне"))
            val ok = getSystemService(PowerManager::class.java).isIgnoringBatteryOptimizations(packageName)
            addView(label(if (ok) "Экономия батареи не мешает серверу" else "Система может выключить сервер в фоне", 14.5f, if (ok) cGreen else cMuted))
            addView(button("Не убивать в фоне") { askBattery() })
            addView(label("На Realme/OPPO включи ещё «Автозапуск» и «Работа в фоне» в настройках приложения.", 13f, cMuted).apply { setPadding(0, dp(8), 0, 0) })
        })
        content.addView(card {
            addView(caption("Подключение"))
            addView(row("Порт игры", ServerService.PORT.toString()))
            addView(divider())
            addView(row("Панель управления", "127.0.0.1:${ServerService.PANEL_PORT}"))
            addView(button("Как подключиться") { howTo() })
            addView(button("Открыть панель в браузере") { openPanel() })
        })
        content.addView(card {
            addView(caption("О приложении"))
            val ver = try { packageManager.getPackageInfo(packageName, 0).versionName ?: "" } catch (_: Exception) { "" }
            addView(row("Версия приложения", ver))
            addView(divider())
            addView(row("Сервер", status?.optString("version")?.takeIf { it.isNotEmpty() }?.let { "WolfCraft $it" } ?: "WolfCraft"))
            addView(label("Тот же WolfServer, что и на ПК (WolfServer.exe).", 13f, cMuted).apply { setPadding(0, dp(8), 0, 0) })
        })
    }

    private fun renderConsole() {
        header("Консоль")
        val serverLog = status?.optJSONArray("log")?.let { a -> (0 until a.length()).joinToString("\n") { a.getString(it) } }
        val text = listOfNotNull(serverLog?.takeIf { it.isNotBlank() }, ServerService.logText().takeIf { it.isNotBlank() })
            .joinToString("\n— процесс —\n")
            .ifBlank { "Пусто. Запусти сервер." }
        content.addView(card {
            addView(caption("Журнал"))
            addView(label(text, 12f, Color.rgb(190, 198, 208)).apply { typeface = Typeface.MONOSPACE; setTextIsSelectable(true) })
        })
        if (ServerService.running()) content.addView(sayCard("Написать в чат игры"))
        scroll.post { scroll.fullScroll(View.FOCUS_DOWN) }
    }

    // ---- Actions ----------------------------------------------------------------------------

    private fun startServer() {
        val prefs = getSharedPreferences("ui", MODE_PRIVATE)
        ServerService.start(this, prefs.getString("name", "Сервер") ?: "Сервер", prefs.getBoolean("creative", false))
    }

    private fun copyAddress() {
        val addr = addresses().firstOrNull() ?: "127.0.0.1:${ServerService.PORT}"
        getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("WolfServer", addr))
        Toast.makeText(this, "Скопировано: $addr", Toast.LENGTH_SHORT).show()
    }

    private fun openPanel() {
        if (!ServerService.running()) {
            Toast.makeText(this, "Сначала запусти сервер", Toast.LENGTH_SHORT).show()
            return
        }
        try {
            startActivity(Intent(Intent.ACTION_VIEW, Uri.parse("http://127.0.0.1:${ServerService.PANEL_PORT}/")))
        } catch (e: Exception) {
            Toast.makeText(this, "Нет браузера: ${e.message}", Toast.LENGTH_LONG).show()
        }
    }

    private fun howTo() {
        AlertDialog.Builder(this)
            .setTitle("Как подключиться")
            .setMessage(
                "1. Главная → «Запустить».\n" +
                    "2. На этом же телефоне: WolfCraft → Играть → Игра по сети → «Рядом» (или «По коду»: 127.0.0.1:${ServerService.PORT}).\n" +
                    "3. Друзья в той же Wi‑Fi сети или на твоей точке доступа вводят адрес с Главной.\n" +
                    "4. На ПК тот же сервер: WolfServer.exe.\n\n" +
                    "Мини-игра «Питомник»: в игре на сервере напиши в чат /питомник."
            )
            .setPositiveButton("Понятно", null)
            .show()
    }

    private fun askBattery() {
        val pm = getSystemService(PowerManager::class.java)
        if (pm.isIgnoringBatteryOptimizations(packageName)) {
            Toast.makeText(this, "Уже разрешено", Toast.LENGTH_SHORT).show()
            return
        }
        try {
            startActivity(Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:$packageName")))
        } catch (_: Exception) {
            startActivity(Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
        }
    }
}
