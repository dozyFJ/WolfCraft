package com.wolfcraft.ota;
import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.widget.TextView;

/** A separate process survives while the old Rust event loop and server exit. */
public final class RestartActivity extends Activity {
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        TextView text = new TextView(this);text.setText("WolfCraft · перезапуск…");text.setTextSize(22);text.setPadding(32,64,32,32);setContentView(text);
        for (String key : new String[]{"main","native"}) {
            int pid=OtaManager.pid(this,key);
            if(pid>0 && pid!=android.os.Process.myPid()) android.os.Process.killProcess(pid);
        }
        new Handler(Looper.getMainLooper()).postDelayed(() -> {
            Intent launch=getPackageManager().getLaunchIntentForPackage(getPackageName());
            if(launch!=null){launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK|Intent.FLAG_ACTIVITY_CLEAR_TASK);startActivity(launch);}
            finish();
        },500);
    }
}
