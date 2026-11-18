package com.wolfcraft.wolfcut;

import android.app.NativeActivity;
import android.content.ClipData;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ConcurrentLinkedQueue;

public final class WolfCutActivity extends NativeActivity {
    private static final int IMPORT = 1;
    private static final int EXPORT = 2;
    private final ConcurrentLinkedQueue<String> results = new ConcurrentLinkedQueue<>();
    private File session;
    private final Updater updater = new Updater(this, this::reportUpdate);

    @Override
    public void onCreate(Bundle state) {
        try {
            session = Files.createTempDirectory(getCacheDir().toPath(), "wolfcut-").toFile();
        } catch (IOException error) {
            throw new IllegalStateException("Could not create editing cache", error);
        }
        super.onCreate(state);
    }

    public String binaryDirectory() {
        return getApplicationInfo().nativeLibraryDir;
    }

    public String sessionDirectory() {
        return session.getAbsolutePath();
    }

    public String pollResult() {
        return results.poll();
    }

    public void checkUpdate() {
        updater.check();
    }

    public void installUpdate() {
        updater.install();
    }

    private void reportUpdate(String status, boolean available) {
        try {
            results.add(new JSONObject()
                    .put("type", "update")
                    .put("status", status)
                    .put("available", available)
                    .toString());
        } catch (JSONException error) {
            throw new IllegalStateException(error);
        }
    }

    public void pickVideos() {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("video/*");
        intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
        open(intent, IMPORT);
    }

    public void saveVideo() {
        Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("video/mp4");
        intent.putExtra(Intent.EXTRA_TITLE, "wolfcut.mp4");
        open(intent, EXPORT);
    }

    private void open(Intent intent, int operation) {
        runOnUiThread(() -> {
            try {
                startActivityForResult(intent, operation);
            } catch (RuntimeException error) {
                if (operation == EXPORT) {
                    new File(session, "wolfcut.mp4").delete();
                }
                report(operation, "Не удалось открыть выбор файла.", new JSONArray());
            }
        });
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != IMPORT && request != EXPORT) {
            return;
        }
        if (result != RESULT_OK || data == null) {
            if (request == EXPORT) {
                new File(session, "wolfcut.mp4").delete();
            }
            report(request, "Отменено.", new JSONArray());
            return;
        }
        if (request == IMPORT) {
            List<Uri> videos = new ArrayList<>();
            ClipData selection = data.getClipData();
            if (selection != null) {
                for (int index = 0; index < selection.getItemCount(); index++) {
                    videos.add(selection.getItemAt(index).getUri());
                }
            } else if (data.getData() != null) {
                videos.add(data.getData());
            }
            new Thread(() -> copyVideos(videos), "wolfcut-import").start();
        } else {
            new Thread(() -> writeExport(data.getData()), "wolfcut-save").start();
        }
    }

    private void copyVideos(List<Uri> videos) {
        JSONArray paths = new JSONArray();
        for (Uri video : videos) {
            File copy = null;
            try (InputStream input = getContentResolver().openInputStream(video)) {
                if (input == null) {
                    continue;
                }
                File directory = Files.createTempDirectory(session.toPath(), "clip-").toFile();
                copy = new File(directory, displayName(video));
                Files.copy(input, copy.toPath());
                paths.put(copy.getAbsolutePath());
            } catch (IOException | RuntimeException error) {
                if (copy != null) {
                    copy.delete();
                }
            }
        }
        report(IMPORT, paths.length() == 0 ? "Не удалось прочитать видео." : "", paths);
    }

    private String displayName(Uri uri) {
        try (Cursor cursor = getContentResolver().query(
                uri, new String[]{OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                String value = cursor.getString(0);
                if (value != null) {
                    String name = new File(value).getName();
                    if (!name.isEmpty() && !name.equals(".") && !name.equals("..")) {
                        return name;
                    }
                }
            }
        }
        return "video.mp4";
    }

    private void writeExport(Uri destination) {
        File staged = new File(session, "wolfcut.mp4");
        String status = "Видео сохранено.";
        try (OutputStream output = getContentResolver().openOutputStream(destination, "wt")) {
            if (output == null) {
                throw new IOException("No output stream");
            }
            Files.copy(staged.toPath(), output);
        } catch (IOException | RuntimeException error) {
            status = "Ошибка сохранения видео.";
        } finally {
            staged.delete();
        }
        report(EXPORT, status, new JSONArray());
    }

    private void report(int operation, String status, JSONArray paths) {
        try {
            results.add(new JSONObject()
                    .put("type", operation == IMPORT ? "import" : "export")
                    .put("status", status)
                    .put("paths", paths)
                    .toString());
        } catch (JSONException error) {
            throw new IllegalStateException(error);
        }
    }
}
