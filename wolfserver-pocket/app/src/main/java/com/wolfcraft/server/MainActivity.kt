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
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import java.net.Inet4Address
import java.net.NetworkInterface

/** Карманный WolfServer: старт/стоп, адреса для игроков, панель управления и логи. */
class MainActivity : Activity() {

    private val handler = Handler(Looper.getMainLooper())
    private lateinit var statusView: TextView
    private lateinit var dot: View
    private lateinit var addrView: TextView
    private lateinit var logView: TextView
    private lateinit var logScroll: ScrollView
    private lateinit var nameEdit: EditText
    private lateinit var creativeBox: CheckBox
    private lateinit var startBtn: Button
    private lateinit var stopBtn: Button

    private val cBg = Color.rgb(17, 19, 23)
    private val cCard = Color.rgb(29, 33, 39)
    private val cText = Color.rgb(232, 235, 240)
    private val cMuted = Color.rgb(140, 148, 160)
    private val cAccent = Color.rgb(61, 220, 132)

    private val tick = object : Runnable {
        override fun run() {
            refresh()
            handler.postDelayed(this, 1000)
        }
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()

    private fun rounded(color: Int, radius: Int = 16) = GradientDrawable().apply {
        setColor(color)
        cornerRadius = dp(radius).toFloat()
    }

    private fun label(t: String, size: Float = 15f, color: Int = cText, bold: Boolean = false) = TextView(this).apply {
        this.text = t
        textSize = size
        setTextColor(color)
        if (bold) typeface = Typeface.DEFAULT_BOLD
    }

    private fun button(t: String, primary: Boolean = false, onClick: () -> Unit) = Button(this).apply {
        this.text = t
        isAllCaps = false
        textSize = 15f
        setTextColor(if (primary) Color.rgb(10, 30, 18) else cText)
        background = rounded(if (primary) cAccent else Color.rgb(44, 50, 58), 22)
        setOnClickListener { onClick() }
        layoutParams = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, dp(50)).apply { topMargin = dp(8) }
    }

    private fun cardLayout() = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        background = rounded(cCard)
        setPadding(dp(16), dp(14), dp(16), dp(14))
        layoutParams = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT).apply { topMargin = dp(12) }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.statusBarColor = cBg
        val prefs = getSharedPreferences("ui", MODE_PRIVATE)

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(18), dp(22), dp(18), dp(24))
        }
        root.addView(label("WolfServer Pocket", 26f, cText, true))
        root.addView(label("Сервер WolfCraft прямо на телефоне — тот же, что на ПК", 13f, cMuted))

        // Status.
        val statusCard = cardLayout()
        val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }
        dot = View(this).apply {
            background = GradientDrawable().apply { shape = GradientDrawable.OVAL; setColor(cMuted) }
            layoutParams = LinearLayout.LayoutParams(dp(12), dp(12)).apply { rightMargin = dp(10) }
        }
        statusView = label("Остановлен", 16f, cText, true)
        row.addView(dot)
        row.addView(statusView)
        statusCard.addView(row)
        addrView = label("", 15f, cAccent).apply { setPadding(0, dp(8), 0, 0); setTextIsSelectable(true) }
        statusCard.addView(addrView)
        root.addView(statusCard)

        // World settings and controls.
        val worldCard = cardLayout()
        worldCard.addView(label("Мир", 13f, cMuted))
        nameEdit = EditText(this).apply {
            setText(prefs.getString("name", "Сервер"))
            setTextColor(cText)
            setHintTextColor(cMuted)
            hint = "Название мира"
            background = rounded(Color.rgb(20, 23, 28), 12)
            setPadding(dp(12), dp(10), dp(12), dp(10))
        }
        worldCard.addView(nameEdit)
        creativeBox = CheckBox(this).apply {
            this.text = "Творческий режим (для нового мира)"
            setTextColor(cText)
            isChecked = prefs.getBoolean("creative", false)
        }
        worldCard.addView(creativeBox)
        startBtn = button("▶  Запустить сервер", primary = true) {
            prefs.edit().putString("name", nameEdit.text.toString()).putBoolean("creative", creativeBox.isChecked).apply()
            ServerService.start(this, nameEdit.text.toString(), creativeBox.isChecked)
        }
        stopBtn = button("■  Остановить") { ServerService.stop(this) }
        worldCard.addView(startBtn)
        worldCard.addView(stopBtn)
        root.addView(worldCard)

        // Tools.
        val tools = cardLayout()
        tools.addView(button("Скопировать адрес") { copyAddress() })
        tools.addView(button("Панель управления") { openPanel() })
        tools.addView(button("Как подключиться") { howTo() })
        tools.addView(button("Не убивать в фоне") { askBattery() })
        root.addView(tools)

        // Log.
        val logCard = cardLayout()
        logCard.addView(label("Журнал сервера", 13f, cMuted))
        logView = label("", 11f, Color.rgb(190, 198, 208)).apply { typeface = Typeface.MONOSPACE; setTextIsSelectable(true) }
        logScroll = ScrollView(this).apply {
            addView(logView)
            layoutParams = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, dp(220))
        }
        logCard.addView(logScroll)
        root.addView(logCard)

        setContentView(ScrollView(this).apply {
            setBackgroundColor(cBg)
            addView(root)
        })

        if (android.os.Build.VERSION.SDK_INT >= 33) {
            requestPermissions(arrayOf("android.permission.POST_NOTIFICATIONS"), 1)
        }
    }

    override fun onResume() {
        super.onResume()
        handler.post(tick)
    }

    override fun onPause() {
        handler.removeCallbacks(tick)
        super.onPause()
    }

    private fun refresh() {
        val on = ServerService.running()
        statusView.text = ServerService.status
        (dot.background as GradientDrawable).setColor(if (on) cAccent else cMuted)
        val ips = lanIps()
        addrView.text = when {
            !on -> ""
            ips.isEmpty() -> "На этом телефоне: 127.0.0.1:${ServerService.PORT}\n(включи Wi‑Fi или точку доступа, чтобы зашли друзья)"
            else -> "Адрес для друзей:\n" + ips.joinToString("\n") { "$it:${ServerService.PORT}" } + "\nНа этом телефоне: 127.0.0.1:${ServerService.PORT}"
        }
        startBtn.isEnabled = !on
        startBtn.alpha = if (on) 0.45f else 1f
        stopBtn.isEnabled = on
        stopBtn.alpha = if (on) 1f else 0.45f
        nameEdit.isEnabled = !on
        val log = ServerService.logText()
        if (logView.text.length != log.length) {
            logView.text = log
            logScroll.post { logScroll.fullScroll(View.FOCUS_DOWN) }
        }
    }

    private fun lanIps(): List<String> = try {
        NetworkInterface.getNetworkInterfaces().toList()
            .filter { it.isUp && !it.isLoopback }
            .flatMap { it.inetAddresses.toList() }
            .filterIsInstance<Inet4Address>()
            .map { it.hostAddress ?: "" }
            .filter { it.isNotEmpty() }
    } catch (_: Exception) {
        emptyList()
    }

    private fun copyAddress() {
        val ip = lanIps().firstOrNull() ?: "127.0.0.1"
        val addr = "$ip:${ServerService.PORT}"
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
                "1. Нажми «Запустить сервер».\n" +
                    "2. На этом же телефоне: открой WolfCraft → Играть → По сети, адрес 127.0.0.1:${ServerService.PORT}.\n" +
                    "3. Друзья в той же Wi‑Fi сети (или на твоей точке доступа) вводят адрес из карточки сверху.\n" +
                    "4. На ПК можно запустить тот же сервер: WolfServer.exe.\n\n" +
                    "Мини-игра «Питомник»: в игре на сервере напиши в чат /питомник."
            )
            .setPositiveButton("Понятно", null)
            .show()
    }

    private fun askBattery() {
        val pm = getSystemService(PowerManager::class.java)
        if (pm.isIgnoringBatteryOptimizations(packageName)) {
            Toast.makeText(this, "Уже разрешено. На Realme/OPPO включи ещё автозапуск в настройках приложения.", Toast.LENGTH_LONG).show()
            return
        }
        try {
            startActivity(Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:$packageName")))
        } catch (_: Exception) {
            startActivity(Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
        }
    }
}
