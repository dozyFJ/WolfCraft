package com.wolfcraft.ota;

import android.app.NativeActivity;
import android.os.Bundle;
import dalvik.system.PathClassLoader;
import java.io.File;

/** Stable APK shell. The Android NativeActivity ABI stays in the shell; Rust lives in .so. */
public class NativeHostActivity extends NativeActivity {
    private ClassLoader runtimeLoader;
    private String kind;
    private String build;
    @Override public ClassLoader getClassLoader() {
        return runtimeLoader == null ? super.getClassLoader() : runtimeLoader;
    }
    @Override protected void onCreate(Bundle state) {
        kind = getPackageName().equals("com.wolfcraft.mobile") ? "game" : "studio";
        OtaManager.registerPid(this, "native");
        File dir = OtaManager.selectRuntime(this, kind, "native");
        if (dir != null) {
            String paths = dir.getAbsolutePath() + File.pathSeparator + getApplicationInfo().nativeLibraryDir;
            runtimeLoader = new PathClassLoader(getApplicationInfo().sourceDir, paths, super.getClassLoader());
        }
        build=OtaManager.loadedBuild(this,kind,"native");
        // Never restore opaque native state created by a different core version.
        Bundle compatible=state!=null&&build.equals(state.getString("wolf.native.build"))?state:null;
        // If native startup aborts, leave the boot marker intact. The next launch rolls back.
        super.onCreate(compatible);
    }
    @Override protected void onSaveInstanceState(Bundle out){super.onSaveInstanceState(out);out.putString("wolf.native.build",build);}
    public void checkForUpdates() { OtaManager.show(this, kind); }
    public void markUpdateHealthy() { OtaManager.markHealthy(this, kind, "native"); }
}
