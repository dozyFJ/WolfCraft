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
 * WolfStudio: the studio and the server in one app. Bottom bar — Студия (opens the WolfCraft
 * studio, also on the running server's world), Сервер (dashboard: address, world, players,
 * start/stop, messages), Консоль (log), Настройки. Server data comes from its own API on
 * 127.0.0.1; the studio is the engine's NativeActivity in its own process.
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
    /** Дашборд: 0 servers (ordinary worlds), 1 places. */
    private var listTab = 0
    /** Сервер: 0 overview, 1 console, 2 settings, 3 plugins, 4 modules, 5 statistics, 6 logs. */
    private var serverTab = 0
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
        // The console moved into «Сервер» (its own tabs: console, settings, plugins…).
        // One app: play, build, host. Главная · Редактор · Сервер · Профиль.
        val items = listOf("⌂" to "Главная", "✎" to "Редактор", "▶" to "Сервер", "☺" to "Профиль")
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
        startPlaceFromStudio()
        polling = true
        Thread { pollLoop() }.start()
        handler.post(tick)
    }

    override fun onPause() {
        polling = false
        handler.removeCallbacks(tick)
        super.onPause()
    }

    private fun renderKey() = "$tab|$serverTab|${ServerService.running()}|${ServerService.status}|${status?.toString()}|${ServerService.logText().length}"

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
            0 -> renderMain()
            1 -> renderStudio()
            2 -> renderServerTab()
            else -> renderProfile()
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

    /** Invite codes from the server (short, typed in «По коду» instead of the address). */
    private fun codes(): List<String> =
        status?.optJSONArray("codes")?.let { a -> (0 until a.length()).map { a.getString(it) } }.orEmpty()

    private fun copyCode() {
        val code = codes().firstOrNull() ?: return copyAddress()
        getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("WolfCraft", code))
        Toast.makeText(this, "Код скопирован: $code", Toast.LENGTH_SHORT).show()
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
            addView(label("WolfStudio" + (ver?.let { " · сервер v$it" } ?: ""), 12.5f, cMuted))
        })
        bar.addView(label(if (on) "РАБОТАЕТ" else "ОСТАНОВЛЕН", 12f, if (on) cGreen else cMuted, true).apply {
            letterSpacing = 0.08f
            background = rounded(Color.TRANSPARENT, 10, if (on) cGreen else cLine)
            setPadding(dp(12), dp(6), dp(12), dp(6))
        })
        content.addView(bar)
    }

    /** A server (place): its folder name, title and world kind. */
    private data class Place(val id: String, val name: String, val kind: String, val made: Long, val isPlace: Boolean)

    private fun placesDir() = java.io.File(filesDir, "wolfserver/worlds").apply { mkdirs() }

    /** Every server world: made here, in the studio or by «Запустить» on the Сервер tab. */
    private fun places(): List<Place> = (placesDir().listFiles() ?: emptyArray()).mapNotNull { d ->
        val toml = java.io.File(d, "world.toml")
        if (!toml.isFile) return@mapNotNull null
        val kv = try {
            toml.readLines().mapNotNull { l ->
                val i = l.indexOf('=')
                if (i <= 0) null else l.substring(0, i).trim() to l.substring(i + 1).trim().trim('"')
            }.toMap()
        } catch (_: Exception) { emptyMap() }
        val kind = if (kv["studio_project"] == "true") "Плейс · " else "Обычный мир · "
        val terrain = when (kv["world_type"]) { "baseplate" -> "Основа"; "flat" -> "Плоский мир"; else -> "Планета" }
        Place(d.name, kv["name"] ?: d.name, kind + terrain, kv["created"]?.toLongOrNull() ?: 0L, kv["studio_project"] == "true")
    }.sortedByDescending { it.made }

    /** Both world profiles share one server runtime, with separate folders and settings. */
    private fun createPlace(name: String, isPlace: Boolean) {
        val now = System.currentTimeMillis() / 1000
        val id = "server-${java.util.UUID.randomUUID()}"
        val dir = java.io.File(placesDir(), id).apply { mkdirs() }
        val q = name.replace("\r", " ").replace("\n", " ").replace("\\", "\\\\").replace("\"", "\\\"")
        val seed = (System.nanoTime() and 0x3fffffff)
        java.io.File(dir, "world.toml").writeText(
            """
            id = "$id"
            name = "$q"
            seed = $seed
            mode = "${if (isPlace) "creative" else "survival"}"
            size = "small"
            world_type = "${if (isPlace) "baseplate" else "standard"}"
            continents = "auto"
            structures = ${!isPlace}
            difficulty = "${if (isPlace) "peaceful" else "normal"}"
            created = $now
            last_played = $now
            generator_version = 1
            format_version = 3
            shape = "sphere"
            studio_project = $isPlace
            """.trimIndent() + "\n"
        )
        render()
    }

    private fun askNewPlace() {
        AlertDialog.Builder(this)
            .setTitle("Что создать?")
            .setItems(arrayOf("Обычный мир — планета и выживание", "Плейс — пустая основа и мини-игры")) { _, type ->
                val isPlace = type == 1
                val name = input("Название", "${if (isPlace) "Плейс" else "Мир"} ${places().size + 1}")
                val box = LinearLayout(this).apply { setPadding(dp(20), dp(8), dp(20), 0); addView(name) }
                AlertDialog.Builder(this)
                    .setTitle(if (isPlace) "Новый плейс" else "Новый обычный мир")
                    .setMessage(if (isPlace) "Пустая основа для своей игры в WolfStudio." else "Планета с природой и выживанием. Запускается тем же WolfServer.")
                    .setView(box)
                    .setPositiveButton("Создать") { _, _ -> name.text.toString().trim().ifEmpty { null }?.let { createPlace(it, isPlace) } }
                    .setNegativeButton("Отмена", null)
                    .show()
            }
            .setNegativeButton("Отмена", null)
            .show()
    }

    /** Opens the editor scene on a place (a running server's world is edited live). */
    private fun editPlace(p: Place) {
        if (isLive(p)) {
            openStudio("127.0.0.1:${ServerService.PORT}")
            return
        }
        try {
            java.io.File(filesDir, "open_place.txt").writeText(p.id)
            java.io.File(filesDir, "open_server.txt").delete()
            startActivity(Intent(this, android.app.NativeActivity::class.java))
        } catch (e: Exception) {
            Toast.makeText(this, "Студия не открылась: ${e.message}", Toast.LENGTH_LONG).show()
        }
    }

    private fun isLive(p: Place) = ServerService.running() &&
        (ServerService.worldId == p.id || (ServerService.worldId.isEmpty() && ServerService.world == p.name))

    private fun runPlace(p: Place) {
        val go = { ServerService.start(this, p.name, true, p.id) }
        if (ServerService.running()) {
            ServerService.stop(this)
            handler.postDelayed({ go() }, 1500)
        } else go()
        Toast.makeText(this, "Запуск сервера «${p.name}»…", Toast.LENGTH_SHORT).show()
    }

    /** Deletes a world or place folder for good (after asking). A running one must stop first. */
    private fun askDelete(p: Place) {
        if (isLive(p)) {
            Toast.makeText(this, "Сначала останови сервер «${p.name}»", Toast.LENGTH_SHORT).show()
            return
        }
        AlertDialog.Builder(this)
            .setTitle("Удалить «${p.name}»?")
            .setMessage(if (p.isPlace) "Плейс со всеми постройками, скриптами и данными удалится без возврата." else "Мир со всеми постройками и вещами игроков удалится без возврата.")
            .setPositiveButton("Удалить") { _, _ ->
                val dir = java.io.File(placesDir(), p.id)
                // Only inside the worlds folder, never above it.
                if (dir.canonicalPath.startsWith(placesDir().canonicalPath + java.io.File.separator) && dir.deleteRecursively()) {
                    Toast.makeText(this, "«${p.name}» удалён", Toast.LENGTH_SHORT).show()
                } else {
                    Toast.makeText(this, "Не удалось удалить «${p.name}»", Toast.LENGTH_LONG).show()
                }
                render()
            }
            .setNegativeButton("Отмена", null)
            .show()
    }

    /** Renames a world or place (its `name` in world.toml). */
    private fun askRename(p: Place) {
        val name = input("Название", p.name)
        val box = LinearLayout(this).apply { setPadding(dp(20), dp(8), dp(20), 0); addView(name) }
        AlertDialog.Builder(this)
            .setTitle("Переименовать")
            .setView(box)
            .setPositiveButton("Готово") { _, _ ->
                val n = name.text.toString().trim().replace("\r", " ").replace("\n", " ")
                if (n.isNotEmpty()) {
                    val f = java.io.File(placesDir(), "${p.id}/world.toml")
                    try {
                        val q = n.replace("\\", "\\\\").replace("\"", "\\\"")
                        f.writeText(f.readLines().joinToString("\n") { if (it.trim().startsWith("name") && it.contains('=')) "name = \"$q\"" else it } + "\n")
                    } catch (_: Exception) {}
                    render()
                }
            }
            .setNegativeButton("Отмена", null)
            .show()
    }

    /** One card: tap to edit, «Запустить»/«Остановить», ⋮ for rename and delete. */
    private fun placeCard(p: Place) {
        val live = isLive(p)
        content.addView(card {
            isClickable = true
            isFocusable = true
            setOnClickListener { editPlace(p) }
            setOnLongClickListener { askDelete(p); true }
            val top = LinearLayout(this@MainActivity).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }
            top.addView(label(p.name, 17f, cText, true).apply { layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f) })
            if (live) top.addView(label("РАБОТАЕТ", 11f, cGreen, true).apply {
                setPadding(dp(10), dp(4), dp(10), dp(4))
                background = rounded(Color.TRANSPARENT, 10, cGreen)
            })
            addView(top)
            addView(label("${p.kind} · нажми, чтобы редактировать${if (live) " (правки сразу у игроков)" else ""}", 13f, cMuted).apply { setPadding(0, dp(4), 0, 0) })
            if (live) codes().firstOrNull()?.let { c -> addView(label("Код для входа: $c", 16f, cAccent, true).apply { setPadding(0, dp(6), 0, 0) }) }
            val row = LinearLayout(this@MainActivity).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }
            val main = if (live) button("■  Остановить", 2) { ServerService.stop(this@MainActivity) } else button("▶  Запустить") { runPlace(p) }
            row.addView(main.apply { layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f) })
            row.addView(button("✎") { askRename(p) }.apply { layoutParams = LinearLayout.LayoutParams(dp(56), ViewGroup.LayoutParams.WRAP_CONTENT).apply { marginStart = dp(8) } })
            row.addView(button("🗑", 2) { askDelete(p) }.apply { layoutParams = LinearLayout.LayoutParams(dp(56), ViewGroup.LayoutParams.WRAP_CONTENT).apply { marginStart = dp(8) } })
            addView(row)
        })
    }

    /** Two-way switch (a pill with a sliding highlight): «Серверы» | «Плейсы». */
    private fun segmented(items: List<String>, selected: Int, onPick: (Int) -> Unit): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        background = rounded(cCard, 24, cLine)
        setPadding(dp(4), dp(4), dp(4), dp(4))
        layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply { topMargin = dp(12) }
        items.forEachIndexed { i, s ->
            addView(TextView(this@MainActivity).apply {
                text = s
                gravity = Gravity.CENTER
                textSize = 15f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor(if (i == selected) Color.WHITE else cMuted)
                background = if (i == selected) rounded(cAccent, 20) else null
                setPadding(0, dp(12), 0, dp(12))
                layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
                setOnClickListener { if (i != selected) onPick(i) }
            })
        }
    }

    /** Дашборд: servers (ordinary worlds) and places (own games, like Roblox Studio) on two tabs. */
    private fun renderStudio() {
        header("Редактор")
        val list = places()
        val placesList = list.filter { it.isPlace }
        val worldsList = list.filter { !it.isPlace }
        content.addView(segmented(listOf("Серверы · ${worldsList.size}", "Плейсы · ${placesList.size}"), listTab) { listTab = it; render() })
        val isPlace = listTab == 1
        content.addView(button(if (isPlace) "＋  Создать плейс" else "＋  Создать сервер", 1) { askNew(isPlace) }.apply {
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply { topMargin = dp(12) }
        })
        val shown = if (isPlace) placesList else worldsList
        if (shown.isEmpty()) {
            content.addView(card {
                addView(label(if (isPlace) "Пока нет плейсов" else "Пока нет серверов", 16f, cText, true))
                addView(label(if (isPlace) "Плейс — твоя игра: пустая основа, скрипты на Luau, мини-игры из Тулбокса. Нажми на карточку — откроется редактор." else "Сервер — обычный мир WolfCraft: планета, природа, выживание. «Запустить» — и друзья заходят по коду.", 14f, cMuted).apply { setPadding(0, dp(6), 0, 0) })
            })
        }
        shown.forEach { placeCard(it) }
        content.addView(label("✎ — переименовать, 🗑 или удержание карточки — удалить.", 13f, cMuted).apply { setPadding(dp(4), dp(14), dp(4), 0) })
    }

    /** New server or place straight from the current tab: just a name. */
    private fun askNew(isPlace: Boolean) {
        val name = input("Название", "${if (isPlace) "Плейс" else "Сервер"} ${places().count { it.isPlace == isPlace } + 1}")
        val box = LinearLayout(this).apply { setPadding(dp(20), dp(8), dp(20), 0); addView(name) }
        AlertDialog.Builder(this)
            .setTitle(if (isPlace) "Новый плейс" else "Новый сервер")
            .setMessage(if (isPlace) "Пустая основа для своей игры в WolfStudio." else "Обычный мир: планета с природой и выживанием.")
            .setView(box)
            .setPositiveButton("Создать") { _, _ -> name.text.toString().trim().ifEmpty { null }?.let { createPlace(it, isPlace) } }
            .setNegativeButton("Отмена", null)
            .show()
    }

    /** Сервер: the dashboard and the server controls on one screen. */
    private val serverTabs = listOf("Обзор", "Консоль", "Настройки", "Плагины", "Модули", "Статистика", "Логи")

    /** Сервер: its own tabs along the top, scrolled sideways. */
    private fun renderServerTab() {
        header("Сервер")
        val bar = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; setPadding(0, dp(4), 0, dp(8)) }
        serverTabs.forEachIndexed { i, s ->
            bar.addView(TextView(this).apply {
                text = s
                textSize = 14f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor(if (i == serverTab) Color.WHITE else cMuted)
                background = if (i == serverTab) rounded(cAccent, 18) else rounded(cCard, 18, cLine)
                setPadding(dp(16), dp(9), dp(16), dp(9))
                layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply { marginEnd = dp(8) }
                setOnClickListener { serverTab = i; render() }
            })
        }
        content.addView(android.widget.HorizontalScrollView(this).apply { isHorizontalScrollBarEnabled = false; addView(bar) })
        when (serverTab) {
            0 -> { if (ServerService.running()) renderHome(withHeader = false); renderServerControls() }
            1 -> renderConsole(withHeader = false)
            2 -> renderServerSettings()
            3 -> renderPlugins()
            4 -> renderModules()
            5 -> renderStats()
            else -> renderLogs()
        }
    }

    // ---- Server environment: the folder whose config, plugins and modules the server uses ---

    private fun isPlaceDir(d: java.io.File) = try {
        java.io.File(d, "world.toml").readLines().any { it.replace(" ", "") == "studio_project=true" }
    } catch (_: Exception) { false }

    /** A place runs from its own folder; ordinary worlds share the server's folder. */
    private fun envRoot(): java.io.File {
        val id = ServerService.worldId
        if (id.isNotEmpty()) {
            val d = java.io.File(placesDir(), id)
            if (isPlaceDir(d)) return d
        }
        return java.io.File(filesDir, "wolfserver").apply { mkdirs() }
    }

    private fun envIsPlace() = envRoot().parentFile?.name == "worlds"

    private fun envConfig() = java.io.File(envRoot(), if (envIsPlace()) "place.toml" else "server.toml")

    private fun envTitle() = if (envIsPlace()) "Плейс «${ServerService.world}»" else "Обычные миры (общие настройки)"

    private fun configLines(): List<String> = try { envConfig().readLines() } catch (_: Exception) { emptyList() }

    private fun configValue(key: String): String? = configLines().firstOrNull { it.trim().startsWith("$key ") || it.trim().startsWith("$key=") }?.substringAfter("=")?.trim()

    /** Sets `key = value` in the config (adds the line if missing). Takes effect on restart. */
    private fun setConfig(key: String, value: String) {
        val lines = configLines().toMutableList()
        val i = lines.indexOfFirst { it.trim().startsWith("$key ") || it.trim().startsWith("$key=") }
        if (i >= 0) lines[i] = "$key = $value" else lines += "$key = $value"
        try { envConfig().writeText(lines.joinToString("\n") + "\n") } catch (e: Exception) {
            Toast.makeText(this, "Не сохранилось: ${e.message}", Toast.LENGTH_LONG).show()
        }
    }

    private fun enabledPlugins(): List<String> = configValue("plugins")?.trim('[', ']')?.split(',')?.map { it.trim().trim('"', '\'') }?.filter { it.isNotEmpty() } ?: emptyList()

    private fun restartHint() = label("Изменения применятся после перезапуска сервера.", 13f, cMuted).apply { setPadding(dp(4), dp(10), dp(4), 0) }

    /** Opens a text file in a simple editor dialog (Lua, TOML). */
    private fun editFile(title: String, f: java.io.File) {
        val box = EditText(this).apply {
            setText(try { f.readText() } catch (_: Exception) { "" })
            typeface = Typeface.MONOSPACE
            textSize = 13f
            setTextColor(cText)
            setBackgroundColor(Color.rgb(12, 14, 18))
            setPadding(dp(12), dp(12), dp(12), dp(12))
            minLines = 12
            gravity = Gravity.TOP
            setHorizontallyScrolling(true)
        }
        AlertDialog.Builder(this)
            .setTitle(title)
            .setView(box)
            .setPositiveButton("Сохранить") { _, _ ->
                try { f.parentFile?.mkdirs(); f.writeText(box.text.toString()); Toast.makeText(this, "Сохранено — перезапусти сервер", Toast.LENGTH_SHORT).show() } catch (e: Exception) { Toast.makeText(this, "Ошибка: ${e.message}", Toast.LENGTH_LONG).show() }
                render()
            }
            .setNegativeButton("Отмена", null)
            .show()
    }

    private fun safeId(s: String) = s.lowercase().map { if (it.isLetterOrDigit() && it.code < 128) it else '_' }.joinToString("").trim('_').take(32)

    private fun renderServerSettings() {
        content.addView(card {
            addView(caption("Окружение"))
            addView(label(envTitle(), 16f, cText, true))
            addView(label(envConfig().absolutePath.substringAfter("files/"), 12.5f, cMuted).apply { typeface = Typeface.MONOSPACE })
        })
        content.addView(card {
            addView(caption("Игроки"))
            val max = input("Максимум игроков", configValue("max_players") ?: "16").apply { inputType = android.text.InputType.TYPE_CLASS_NUMBER }
            addView(max)
            addView(button("Сохранить", 1) {
                val n = max.text.toString().toIntOrNull()?.coerceIn(1, 64)
                if (n == null) Toast.makeText(this@MainActivity, "Нужно число от 1 до 64", Toast.LENGTH_SHORT).show()
                else { setConfig("max_players", n.toString()); max.clearFocus(); Toast.makeText(this@MainActivity, "Сохранено", Toast.LENGTH_SHORT).show() }
            })
        })
        content.addView(card {
            addView(caption("Ядро"))
            addView(row("Скрипт ядра", configValue("core")?.trim('"') ?: "builtin"))
            addView(button("Изменить ядро (core/main.lua)") { editFile("core/main.lua", java.io.File(envRoot(), "core/main.lua")) })
        })
        content.addView(card {
            addView(caption("Ключи"))
            addView(label("Ключ владельца хранится в файле настроек и не показывается здесь. Редакторов добавляйте ключами в editor_keys.", 13.5f, cMuted))
        })
        content.addView(restartHint())
    }

    private fun renderPlugins() {
        val dir = java.io.File(envRoot(), "plugins").apply { mkdirs() }
        val on = enabledPlugins()
        content.addView(button("＋  Новый плагин", 1) {
            val name = input("ID плагина (латиницей)", "welcome")
            val box = LinearLayout(this).apply { setPadding(dp(20), dp(8), dp(20), 0); addView(name) }
            AlertDialog.Builder(this).setTitle("Новый плагин").setView(box)
                .setPositiveButton("Создать") { _, _ ->
                    val id = safeId(name.text.toString())
                    if (id.isNotEmpty()) {
                        val d = java.io.File(dir, id).apply { mkdirs() }
                        java.io.File(d, "plugin.toml").writeText("id = \"$id\"\nentry = \"main.lua\"\nmodules = []\n")
                        java.io.File(d, "main.lua").writeText("-- Плагин $id: события start, stop, tick, join, leave, chat, block_edit\non(\"join\", function(player)\n    server.broadcast(\"Привет, \" .. player.nick .. \"!\")\nend)\n")
                        setConfig("plugins", "[" + (on + id).distinct().joinToString(", ") { "\"$it\"" } + "]")
                    }
                    render()
                }.setNegativeButton("Отмена", null).show()
        }.apply { layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT) })
        val list = (dir.listFiles() ?: emptyArray()).filter { java.io.File(it, "plugin.toml").isFile }.sortedBy { it.name }
        if (list.isEmpty()) content.addView(card {
            addView(label("Плагинов нет", 16f, cText, true))
            addView(label("Плагин — серверный скрипт на Luau: приветствия, правила, магазины, защита построек. Работает на сервере для всех игроков.", 14f, cMuted).apply { setPadding(0, dp(6), 0, 0) })
        })
        for (d in list) {
            val id = d.name
            val enabled = id in on
            content.addView(card {
                val top = LinearLayout(this@MainActivity).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }
                top.addView(label(id, 17f, cText, true).apply { layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f) })
                top.addView(label(if (enabled) "ВКЛЮЧЁН" else "ВЫКЛЮЧЕН", 11f, if (enabled) cGreen else cMuted, true).apply {
                    setPadding(dp(10), dp(4), dp(10), dp(4)); background = rounded(Color.TRANSPARENT, 10, if (enabled) cGreen else cLine)
                })
                addView(top)
                val row = LinearLayout(this@MainActivity).apply { orientation = LinearLayout.HORIZONTAL }
                row.addView(button(if (enabled) "Выключить" else "Включить", if (enabled) 0 else 1) {
                    val next = if (enabled) on - id else (on + id).distinct()
                    setConfig("plugins", "[" + next.joinToString(", ") { "\"$it\"" } + "]")
                    render()
                }.apply { layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f) })
                row.addView(button("Код") { editFile("$id/main.lua", java.io.File(d, "main.lua")) }.apply { layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f).apply { marginStart = dp(8) } })
                row.addView(button("🗑", 2) {
                    AlertDialog.Builder(this@MainActivity).setTitle("Удалить плагин «$id»?").setPositiveButton("Удалить") { _, _ ->
                        d.deleteRecursively(); setConfig("plugins", "[" + (on - id).joinToString(", ") { "\"$it\"" } + "]"); render()
                    }.setNegativeButton("Отмена", null).show()
                }.apply { layoutParams = LinearLayout.LayoutParams(dp(56), ViewGroup.LayoutParams.WRAP_CONTENT).apply { marginStart = dp(8) } })
                addView(row)
            })
        }
        content.addView(restartHint())
    }

    private fun renderModules() {
        val dir = java.io.File(envRoot(), "modules").apply { mkdirs() }
        content.addView(button("＋  Новый модуль", 1) {
            val name = input("ID модуля (латиницей)", "utils")
            val box = LinearLayout(this).apply { setPadding(dp(20), dp(8), dp(20), 0); addView(name) }
            AlertDialog.Builder(this).setTitle("Новый модуль").setView(box)
                .setPositiveButton("Создать") { _, _ ->
                    val id = safeId(name.text.toString())
                    if (id.isNotEmpty()) {
                        val f = java.io.File(dir, "$id.lua")
                        if (!f.exists()) f.writeText("-- Модуль $id: подключается в плагине через require(\"$id\")\nlocal M = {}\n\nfunction M.hello(nick)\n    return \"Привет, \" .. nick\nend\n\nreturn M\n")
                    }
                    render()
                }.setNegativeButton("Отмена", null).show()
        }.apply { layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT) })
        val list = (dir.listFiles() ?: emptyArray()).filter { it.isFile && it.name.endsWith(".lua") }.sortedBy { it.name }
        if (list.isEmpty()) content.addView(card {
            addView(label("Модулей нет", 16f, cText, true))
            addView(label("Модуль — общий код для плагинов: require(\"имя\") в плагине.", 14f, cMuted).apply { setPadding(0, dp(6), 0, 0) })
        })
        for (f in list) {
            content.addView(card {
                isClickable = true
                setOnClickListener { editFile(f.name, f) }
                addView(label(f.name.removeSuffix(".lua"), 17f, cText, true))
                addView(label("${f.length()} байт · нажми, чтобы править", 13f, cMuted))
                addView(button("🗑", 2) {
                    AlertDialog.Builder(this@MainActivity).setTitle("Удалить модуль «${f.name}»?").setPositiveButton("Удалить") { _, _ -> f.delete(); render() }.setNegativeButton("Отмена", null).show()
                })
            })
        }
        content.addView(restartHint())
    }

    private fun folderSize(d: java.io.File): Long = d.walkTopDown().filter { it.isFile }.sumOf { it.length() }

    private fun renderStats() {
        val s = status
        val on = ServerService.running()
        val players = s?.optJSONArray("players")?.length() ?: 0
        val root = envRoot()
        val world = if (ServerService.worldId.isNotEmpty()) java.io.File(placesDir(), ServerService.worldId) else null
        val rt = Runtime.getRuntime()
        content.addView(card {
            addView(caption("Сейчас"))
            val rows = listOf(
                "Состояние" to (if (on) "работает" else "остановлен"),
                "Игроков" to (if (on) "$players из ${configValue("max_players") ?: "16"}" else "—"),
                "Работает" to (s?.optLong("uptime")?.let { uptime(it) } ?: "—"),
                "Изменено блоков" to (s?.optInt("edits")?.toString() ?: "—"),
                "Плагинов" to "${enabledPlugins().size} вкл. из ${(java.io.File(root, "plugins").listFiles() ?: emptyArray()).count { it.isDirectory }}",
                "Модулей" to "${(java.io.File(root, "modules").listFiles() ?: emptyArray()).count { it.name.endsWith(".lua") }}",
                "Мир на диске" to (world?.let { "%.1f МБ".format(folderSize(it) / 1048576.0) } ?: "—"),
                "Память приложения" to "%.0f МБ".format((rt.totalMemory() - rt.freeMemory()) / 1048576.0),
            )
            rows.forEachIndexed { i, (k, v) -> addView(row(k, v)); if (i < rows.size - 1) addView(divider()) }
        })
        if (on) playersCard()
    }

    private fun renderLogs() {
        val serverLog = status?.optJSONArray("log")?.let { a -> (0 until a.length()).joinToString("\n") { a.getString(it) } }
        content.addView(card {
            addView(caption("Журнал сервера"))
            addView(label(serverLog?.takeIf { it.isNotBlank() } ?: "Пусто. Запусти сервер.", 12f, Color.rgb(190, 198, 208)).apply { typeface = Typeface.MONOSPACE; setTextIsSelectable(true) })
        })
        content.addView(card {
            addView(caption("Процесс"))
            addView(label(ServerService.logText().takeIf { it.isNotBlank() } ?: "Пусто.", 12f, Color.rgb(160, 168, 178)).apply { typeface = Typeface.MONOSPACE; setTextIsSelectable(true) })
        })
        content.addView(button("Скопировать журнал") {
            getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("WolfServer log", (serverLog ?: "") + "\n" + ServerService.logText()))
            Toast.makeText(this, "Журнал скопирован", Toast.LENGTH_SHORT).show()
        })
    }

    private fun renderHome(withHeader: Boolean = true) {
        if (withHeader) header("Сервер")
        val s = status
        val on = ServerService.running()
        if (on) content.addView(card {
            addView(caption("Код для входа"))
            val cs = codes()
            if (cs.isEmpty()) addView(label("Код появится, когда телефон в Wi‑Fi или раздаёт точку доступа", 14f, cMuted))
            cs.forEach { c ->
                addView(label(c, 40f, cText, true).apply {
                    letterSpacing = 0.12f
                    typeface = Typeface.create(Typeface.MONOSPACE, Typeface.BOLD)
                    setTextIsSelectable(true)
                })
            }
            addView(label("Друзья: WolfCraft → Миры → Игра по сети → «По коду». В одной сети с тобой.", 13.5f, cMuted).apply { setPadding(0, dp(6), 0, 0) })
            if (cs.isNotEmpty()) addView(button("Скопировать код", 1) { copyCode() })
        })
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
                    "Тип" to (when (s?.optString("kind")) { "place" -> "Плейс"; "world" -> "Обычный мир"; else -> "…" }),
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

    private fun renderServerControls() {
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
        if (on) content.addView(sayCard("Сообщение всем игрокам"))
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

    /** Главная: play the game, jump to the editor or the server. */
    private fun renderMain() {
        header("WolfCraft")
        content.addView(card {
            background = rounded(Color.rgb(28, 52, 110), 20)
            addView(label("Играть", 24f, Color.WHITE, true))
            addView(label("Свои миры, сервера друзей по коду, мини-игры. Ник: ${nickname()}", 14f, Color.rgb(200, 212, 240)).apply { setPadding(0, dp(6), 0, dp(10)) })
            addView(button("▶  Играть", 1) { playGame() })
        })
        content.addView(card {
            addView(caption("Создавать"))
            addView(label("Плейсы и сервера в редакторе — как в Roblox Studio, только на телефоне.", 14f, cMuted).apply { setPadding(0, 0, 0, dp(8)) })
            addView(button("✎  Открыть редактор") { selectTab(1) })
        })
        content.addView(card {
            addView(caption("Мой сервер"))
            val on = ServerService.running()
            addView(label(if (on) "Работает: «${ServerService.world}»" else "Остановлен", 15f, if (on) cGreen else cMuted, true))
            if (on) codes().firstOrNull()?.let { addView(row("Код входа", it)) }
            addView(button(if (on) "Управлять сервером" else "Запустить сервер") { selectTab(2) })
        })
    }

    /** Starts the game itself (the engine in game mode, its own worlds and menus). */
    private fun playGame() {
        try {
            java.io.File(filesDir, "open_game.txt").writeText("1")
            java.io.File(filesDir, "open_place.txt").delete()
            java.io.File(filesDir, "open_server.txt").delete()
            startActivity(Intent(this, android.app.NativeActivity::class.java))
        } catch (e: Exception) {
            Toast.makeText(this, "Игра не запустилась: ${e.message}", Toast.LENGTH_LONG).show()
        }
    }

    // The game's own settings file (shared by the game and the editor).
    private fun settingsFile() = java.io.File(filesDir, "settings.toml")

    private fun nickname(): String = try {
        settingsFile().readLines().firstOrNull { it.trim().startsWith("nickname") }?.substringAfter("=")?.trim()?.trim('"')?.ifBlank { null } ?: "Игрок"
    } catch (_: Exception) { "Игрок" }

    private fun setNickname(n: String) {
        val clean = n.trim().replace("\"", "").replace("\\", "").take(16)
        val f = settingsFile()
        val lines = (if (f.exists()) f.readLines() else emptyList()).toMutableList()
        val i = lines.indexOfFirst { it.trim().startsWith("nickname") }
        val line = "nickname = \"$clean\""
        // Top-level keys go before any [section].
        if (i >= 0) lines[i] = line else lines.add(0, line)
        f.writeText(lines.joinToString("\n") + "\n")
    }

    /** Профиль: nickname (used in the game, the editor and on servers) and app settings. */
    private fun renderProfile() {
        header("Профиль")
        content.addView(card {
            addView(caption("Игрок"))
            val nick = input("Ник", nickname())
            addView(nick)
            addView(button("Сохранить ник", 1) {
                setNickname(nick.text.toString())
                nick.clearFocus()
                Toast.makeText(this@MainActivity, "Ник: ${nickname()}", Toast.LENGTH_SHORT).show()
            })
            addView(label("Скин выбирается в игре: Играть → Профиль.", 13f, cMuted).apply { setPadding(0, dp(8), 0, 0) })
        })
        renderSettings(false)
    }

    private fun renderSettings(withHeader: Boolean = true) {
        if (withHeader) header("Настройки")
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
            addView(label("Студия и сервер в одном приложении. Сервер тот же, что и на ПК (WolfServer.exe).", 13f, cMuted).apply { setPadding(0, dp(8), 0, 0) })
        })
    }

    /** Console: the last lines and a command line (text → chat, /kick nick, /stop, /list). */
    private fun renderConsole(withHeader: Boolean = true) {
        if (withHeader) header("Консоль")
        val lines = status?.optJSONArray("log")?.let { a -> (0 until a.length()).map { a.getString(it) } } ?: emptyList()
        val text = lines.takeLast(40).joinToString("\n").ifBlank { if (ServerService.running()) "…" else "Сервер остановлен. Запусти его на вкладке «Обзор»." }
        content.addView(card {
            addView(label(text, 12f, Color.rgb(190, 198, 208)).apply { typeface = Typeface.MONOSPACE; setTextIsSelectable(true) })
        })
        if (ServerService.running()) content.addView(card {
            addView(caption("Команда"))
            val cmd = input("Текст в чат, /kick ник, /list, /stop")
            addView(cmd)
            addView(button("Выполнить", 1) {
                val c = cmd.text.toString().trim()
                cmd.setText("")
                cmd.clearFocus()
                when {
                    c.isEmpty() -> {}
                    c == "/stop" -> ServerService.stop(this@MainActivity)
                    c == "/list" -> {
                        val ps = status?.optJSONArray("players")
                        val names = (0 until (ps?.length() ?: 0)).joinToString(", ") { ps!!.getJSONObject(it).optString("nick") }
                        Toast.makeText(this@MainActivity, if (names.isEmpty()) "Никого нет" else names, Toast.LENGTH_LONG).show()
                    }
                    c.startsWith("/kick ") -> {
                        val nick = c.removePrefix("/kick ").trim()
                        val ps = status?.optJSONArray("players")
                        val id = (0 until (ps?.length() ?: 0)).map { ps!!.getJSONObject(it) }.firstOrNull { it.optString("nick").equals(nick, true) }?.optInt("id")
                        if (id != null) apiAsync("POST", "/api/kick", "&id=$id") else Toast.makeText(this@MainActivity, "Нет игрока «$nick»", Toast.LENGTH_SHORT).show()
                    }
                    else -> apiAsync("POST", "/api/say", "&text=" + URLEncoder.encode(c, "UTF-8"))
                }
            })
        })
        scroll.post { scroll.fullScroll(View.FOCUS_DOWN) }
    }

    // ---- Actions ----------------------------------------------------------------------------

    /** Starts the studio; with `server` it opens that server's world (address) right away. */
    private fun openStudio(server: String?) {
        try {
            val f = java.io.File(filesDir, "open_server.txt")
            if (server != null) f.writeText(server) else f.delete()
            val access = java.io.File(filesDir, "open_access.json")
            if (server == "127.0.0.1:${ServerService.PORT}" && ServerService.running()) {
                access.writeText(JSONObject().put("address", server).put("key", ServerService.token).toString())
            } else access.delete()
            startActivity(Intent(this, android.app.NativeActivity::class.java))
        } catch (e: Exception) {
            Toast.makeText(this, "Студия не открылась: ${e.message}", Toast.LENGTH_LONG).show()
        }
    }

    /** «Запустить» on a place in the studio leaves its folder name here: run it as the server. */
    private fun startPlaceFromStudio() {
        val f = java.io.File(filesDir, "start_server.txt")
        val id = try { f.readText().trim() } catch (_: Exception) { return }
        f.delete()
        if (id.isEmpty()) return
        val name = try {
            java.io.File(filesDir, "wolfserver/worlds/$id/world.toml").readLines()
                .firstOrNull { it.trim().startsWith("name") }?.substringAfter("=")?.trim()?.trim('"') ?: id
        } catch (_: Exception) { id }
        val go = { ServerService.start(this, name, true, id) }
        if (ServerService.running()) {
            ServerService.stop(this)
            handler.postDelayed({ go() }, 1500)
        } else go()
        selectTab(1)
        Toast.makeText(this, "Запуск сервера «$name»…", Toast.LENGTH_SHORT).show()
    }

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

