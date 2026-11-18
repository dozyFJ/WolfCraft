package com.wolfcraft.wolfcut;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;

import java.io.File;
import java.io.FileNotFoundException;

public final class UpdateProvider extends ContentProvider {
    static final String FILE_NAME = "WolfCut-arm64.apk";

    @Override public boolean onCreate() { return true; }

    private File file(Uri uri) {
        if (!("/" + FILE_NAME).equals(uri.getPath())) {
            throw new IllegalArgumentException("Unknown update path");
        }
        return new File(getContext().getCacheDir(), FILE_NAME);
    }

    @Override public String getType(Uri uri) {
        file(uri);
        return "application/vnd.android.package-archive";
    }

    @Override public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        if (!"r".equals(mode)) {
            throw new FileNotFoundException("Update is read-only");
        }
        return ParcelFileDescriptor.open(file(uri), ParcelFileDescriptor.MODE_READ_ONLY);
    }

    @Override public Cursor query(Uri uri, String[] projection,
                                  String selection, String[] arguments, String order) {
        File file = file(uri);
        String[] columns = projection != null ? projection :
                new String[]{OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE};
        MatrixCursor cursor = new MatrixCursor(columns, 1);
        MatrixCursor.RowBuilder row = cursor.newRow();
        for (String column : columns) {
            if (OpenableColumns.DISPLAY_NAME.equals(column)) {
                row.add(FILE_NAME);
            } else if (OpenableColumns.SIZE.equals(column)) {
                row.add(file.length());
            } else {
                row.add(null);
            }
        }
        return cursor;
    }

    @Override public Uri insert(Uri uri, ContentValues values) { throw new UnsupportedOperationException(); }
    @Override public int delete(Uri uri, String selection, String[] arguments) { throw new UnsupportedOperationException(); }
    @Override public int update(Uri uri, ContentValues values, String selection, String[] arguments) { throw new UnsupportedOperationException(); }
}
