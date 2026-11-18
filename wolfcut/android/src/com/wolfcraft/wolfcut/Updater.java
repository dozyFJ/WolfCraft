package com.wolfcraft.wolfcut;

import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.provider.Settings;

import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.concurrent.atomic.AtomicBoolean;

import javax.net.ssl.HttpsURLConnection;

final class Updater {
    private static final String REPOSITORY = "https://raw.githubusercontent.com/dozyFJ/WolfCraft/";
    private static final String CHANNEL = REPOSITORY + "wolfcut/wolfcut/android/update.json";
    private static final String APK = "WolfCut-arm64.apk";
    private static final String MIME = "application/vnd.android.package-archive";
    private static final long MAX_APK = 100L * 1024 * 1024;

    interface Listener {
        void result(String status, boolean available);
    }

    private final Activity activity;
    private final Listener listener;
    private final AtomicBoolean busy = new AtomicBoolean();
    private volatile Release available;

    private static final class Release {
        final int version;
        final String name;
        final String tag;
        final byte[] digest;

        Release(JSONObject json) throws Exception {
            version = json.getInt("versionCode");
            name = json.getString("versionName");
            if (!name.matches("[0-9]+\\.[0-9]+\\.[0-9]+") || version < 1) {
                throw new IOException("Invalid version");
            }
            tag = "wolfcut-v" + name;
            String hash = json.getString("sha256");
            if (!hash.matches("[0-9a-fA-F]{64}")) {
                throw new IOException("Invalid digest");
            }
            digest = new byte[32];
            for (int i = 0; i < digest.length; i++) {
                digest[i] = (byte) Integer.parseInt(hash.substring(i * 2, i * 2 + 2), 16);
            }
        }
    }

    Updater(Activity activity, Listener listener) {
        this.activity = activity;
        this.listener = listener;
    }

    void check() {
        if (!busy.compareAndSet(false, true)) {
            return;
        }
        new Thread(() -> {
            String status;
            try {
                ByteArrayOutputStream metadata = new ByteArrayOutputStream();
                download(CHANNEL, metadata, 8192, null);
                Release release = new Release(new JSONObject(
                        new String(metadata.toByteArray(), StandardCharsets.UTF_8)));
                available = release.version > installed().versionCode ? release : null;
                status = available == null ? "Установлена актуальная версия." :
                        "Доступен WolfCut " + release.name + ".";
            } catch (Exception error) {
                status = "Не удалось проверить обновления.";
            }
            busy.set(false);
            listener.result(status, available != null);
        }, "wolfcut-update-check").start();
    }

    void install() {
        Release release = available;
        if (release == null) {
            check();
            return;
        }
        if (!busy.compareAndSet(false, true)) {
            return;
        }
        if (!activity.getPackageManager().canRequestPackageInstalls()) {
            activity.runOnUiThread(() -> {
                String status = "Разрешите установку для WolfCut и снова нажмите «Обновить».";
                try {
                    activity.startActivity(new Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                            Uri.parse("package:" + activity.getPackageName())));
                } catch (RuntimeException error) {
                    status = "Не удалось открыть настройки установки.";
                }
                busy.set(false);
                listener.result(status, true);
            });
            return;
        }
        new Thread(() -> prepare(release), "wolfcut-update-download").start();
    }

    private void prepare(Release release) {
        File staging = new File(activity.getCacheDir(), "wolfcut-update.part.apk");
        File apk = new File(activity.getCacheDir(), UpdateProvider.FILE_NAME);
        try {
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            try (FileOutputStream output = new FileOutputStream(staging)) {
                download(REPOSITORY + release.tag + "/" + APK, output, MAX_APK, digest);
            }
            if (!MessageDigest.isEqual(release.digest, digest.digest())) {
                throw new IOException("APK checksum differs");
            }
            PackageInfo current = installed();
            PackageInfo candidate = activity.getPackageManager().getPackageArchiveInfo(
                    staging.getAbsolutePath(), PackageManager.GET_SIGNATURES);
            if (candidate == null || !current.packageName.equals(candidate.packageName)
                    || candidate.versionCode != release.version
                    || candidate.versionCode <= current.versionCode
                    || current.signatures == null || current.signatures.length == 0
                    || !Arrays.equals(current.signatures, candidate.signatures)) {
                throw new IOException("APK identity differs");
            }
            if (!staging.renameTo(apk)) {
                throw new IOException("Could not stage APK");
            }
            activity.runOnUiThread(() -> {
                String status = "Подтвердите обновление в системном установщике.";
                try {
                    Uri uri = Uri.parse("content://" + activity.getPackageName() + ".update/" + APK);
                    Intent installer = new Intent(Intent.ACTION_VIEW)
                            .setDataAndType(uri, MIME)
                            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
                    activity.startActivity(installer);
                } catch (RuntimeException error) {
                    status = "Не удалось открыть системный установщик.";
                }
                busy.set(false);
                listener.result(status, true);
            });
        } catch (Exception error) {
            staging.delete();
            busy.set(false);
            listener.result("Не удалось загрузить или проверить APK.", true);
        }
    }

    @SuppressWarnings("deprecation")
    private PackageInfo installed() throws PackageManager.NameNotFoundException {
        return activity.getPackageManager().getPackageInfo(
                activity.getPackageName(), PackageManager.GET_SIGNATURES);
    }

    private static void download(String url, OutputStream output, long limit, MessageDigest digest)
            throws IOException {
        HttpsURLConnection connection = (HttpsURLConnection) new URL(url).openConnection();
        connection.setInstanceFollowRedirects(false);
        connection.setConnectTimeout(12_000);
        connection.setReadTimeout(20_000);
        connection.setUseCaches(false);
        try {
            if (connection.getResponseCode() != 200 || connection.getContentLengthLong() > limit) {
                throw new IOException("Invalid response");
            }
            try (InputStream input = connection.getInputStream()) {
                byte[] buffer = new byte[32_768];
                long total = 0;
                for (int count; (count = input.read(buffer)) != -1;) {
                    total += count;
                    if (total > limit) {
                        throw new IOException("Response too large");
                    }
                    output.write(buffer, 0, count);
                    if (digest != null) {
                        digest.update(buffer, 0, count);
                    }
                }
            }
        } finally {
            connection.disconnect();
        }
    }
}
