package com.wolfcraft.ota;

import android.app.*;
import android.content.*;
import android.content.pm.*;
import android.os.*;
import android.text.InputType;
import android.util.AtomicFile;
import android.widget.*;
import java.io.*;
import java.net.*;
import java.nio.channels.*;
import java.nio.charset.StandardCharsets;
import java.security.PublicKey;
import java.security.cert.CertificateFactory;
import java.util.*;
import java.util.concurrent.atomic.AtomicBoolean;
import org.json.*;

/** Shared OTA shell for the game and studio. No GitHub credential is compiled into an APK. */
public final class OtaManager {
    private static final String REPO="https://api.github.com/repos/dozyFJ/WolfCraft";
    private static final AtomicBoolean busy=new AtomicBoolean();
    private static final Map<String,File> selected=new HashMap<>();
    private static final Map<String,String> selectedBuild=new HashMap<>();
    private static final String BUILTIN="builtin";
    public static android.content.SharedPreferences prefs(Context c){return c.getSharedPreferences("wolf-ota",Context.MODE_PRIVATE);}
    private static File root(Context c){File f=new File(c.getFilesDir(),"ota");f.mkdirs();return f;}
    private static File pointer(Context c,String kind){return new File(root(c),"active-"+kind+".json");}
    private static File pending(Context c,String kind,String part){return new File(root(c),"pending-"+kind+"-"+part);}
    private static byte[] read(File f,int limit) throws Exception {try(InputStream in=new AtomicFile(f).openRead()){return readLimited(in,limit);}}
    private static byte[] readLimited(InputStream in,int limit)throws Exception{ByteArrayOutputStream out=new ByteArrayOutputStream();byte[] b=new byte[8192];int n;while((n=in.read(b))!=-1){if(out.size()+n>limit)throw new IOException("Превышен размер файла");out.write(b,0,n);}return out.toByteArray();}
    private static void write(File f,byte[] bytes)throws Exception {AtomicFile a=new AtomicFile(f);FileOutputStream out=a.startWrite();try{out.write(bytes);a.finishWrite(out);}catch(Exception e){a.failWrite(out);throw e;}}
    private static JSONObject active(Context c,String kind){try{return new JSONObject(new String(read(pointer(c,kind),8192),StandardCharsets.UTF_8));}catch(Exception e){return new JSONObject();}}
    private static PublicKey publicKey(Context c)throws Exception{
        byte[] cert;
        if(Build.VERSION.SDK_INT>=28){PackageInfo info=c.getPackageManager().getPackageInfo(c.getPackageName(),PackageManager.GET_SIGNING_CERTIFICATES);cert=info.signingInfo.getApkContentsSigners()[0].toByteArray();}
        else{PackageInfo info=c.getPackageManager().getPackageInfo(c.getPackageName(),PackageManager.GET_SIGNATURES);cert=info.signatures[0].toByteArray();}
        return CertificateFactory.getInstance("X.509").generateCertificate(new ByteArrayInputStream(cert)).getPublicKey();
    }
    private static JSONObject verify(Context c,String kind,File dir)throws Exception{
        JSONObject m=BundleVerifier.manifest(read(new File(dir,"manifest.json"),65536),read(new File(dir,"manifest.sig"),8192),publicKey(c),kind);
        if(m.getInt("save_format")!=saveFormat(c))throw new SecurityException("Другой формат сохранений");
        if(!m.getString("shell_hash").equals(metadata(c,"wolf.ota.shellhash","development")))throw new SecurityException("Ядро требует другой APK-оболочки");
        if(!dir.getName().equals(m.getString("build")))throw new SecurityException("Неверная папка версии");BundleVerifier.verifyDirectory(dir,m);return m;
    }
    public static void registerPid(Context c,String part){try{write(new File(root(c),"pid-"+part),Integer.toString(android.os.Process.myPid()).getBytes(StandardCharsets.UTF_8));}catch(Exception ignored){} }
    public static int pid(Context c,String part){try{return Integer.parseInt(new String(read(new File(root(c),"pid-"+part),32),StandardCharsets.UTF_8));}catch(Exception e){return 0;}}
    private static String metadata(Context c,String key,String fallback){try{Bundle b=c.getPackageManager().getApplicationInfo(c.getPackageName(),PackageManager.GET_META_DATA).metaData;return b==null?fallback:String.valueOf(b.containsKey(key)?b.get(key):fallback);}catch(Exception e){return fallback;}}
    public static synchronized File selectRuntime(Context c,String kind,String part){
        String cache=kind+":"+part;if(selectedBuild.containsKey(cache))return selected.get(cache);
        JSONObject a=active(c,kind);String build=a.optString("build",BUILTIN);File dir=null;
        if(!build.equals(BUILTIN)){
            try{
                if(!build.matches("[0-9a-f]{40}"))throw new SecurityException("Неверная версия");
                File marker=pending(c,kind,part);
                if(marker.exists()&&build.equals(new String(read(marker,80),StandardCharsets.UTF_8))) {
                    rollback(c,kind,"Предыдущая версия не завершила запуск — восстановлена прошлая версия");
                    a=active(c,kind);build=a.optString("build",BUILTIN);
                }
                if(!build.equals(BUILTIN)){
                    dir=new File(new File(root(c),kind),build);JSONObject manifest=verify(c,kind,dir);
                    // A newly installed APK is authoritative over an older downloaded core.
                    if(manifest.getLong("sequence")<=shellSequence(c)) {
                        new AtomicFile(pointer(c,kind)).delete();
                        for(String component:new String[]{"native","server"})new AtomicFile(pending(c,kind,component)).delete();
                        dir=null;build=BUILTIN;
                    } else write(pending(c,kind,part),build.getBytes(StandardCharsets.UTF_8));
                }
            }catch(Exception e){rollback(c,kind,"Не удалось проверить установленную версию: "+e.getMessage());dir=null;build=BUILTIN;}
        }
        selected.put(cache,dir);selectedBuild.put(cache,build);return dir;
    }
    public static synchronized void markHealthy(Context c,String kind,String part){
        String build=selectedBuild.get(kind+":"+part);if(build==null||build.equals(BUILTIN))return;
        File marker=pending(c,kind,part);try{if(build.equals(new String(read(marker,80),StandardCharsets.UTF_8)))new AtomicFile(marker).delete();}catch(Exception ignored){}
    }
    public static synchronized void rollback(Context c,String kind,String reason){
        JSONObject old=active(c,kind);String previous=old.optString("previous",BUILTIN);
        try{
            if(!previous.equals(BUILTIN)){File dir=new File(new File(root(c),kind),previous);verify(c,kind,dir);}
            write(pointer(c,kind),new JSONObject().put("build",previous).put("previous",BUILTIN).toString().getBytes(StandardCharsets.UTF_8));
        }catch(Exception e){new AtomicFile(pointer(c,kind)).delete();}
        for(String p:new String[]{"native","server"})new AtomicFile(pending(c,kind,p)).delete();
        prefs(c).edit().putString("last_error",reason).apply();
    }
    /** Loads the real server as a shared library: Android forbids exec() of downloaded files. */
    public static synchronized void loadServer(Context c){
        File dir=selectRuntime(c,"studio","server");String base=dir==null?c.getApplicationInfo().nativeLibraryDir:dir.getAbsolutePath();
        File cpp=new File(base,"libc++_shared.so");if(cpp.exists())System.load(cpp.getAbsolutePath());
        System.load(new File(base,"libwolfserver_runtime.so").getAbsolutePath());
    }
    public static void show(Activity activity,String kind){activity.runOnUiThread(()->{
        if(activity.isFinishing())return;
        if(busy.get()){Toast.makeText(activity,"Обновление уже проверяется",Toast.LENGTH_SHORT).show();return;}
        String previous=prefs(activity).getString("last_error","");
        String text=(kind.equals("game")?"WolfCraft":"WolfStudio")+" · обновление без APK\n"+
            "Текущая версия: "+currentBuild(activity,kind).substring(0,Math.min(10,currentBuild(activity,kind).length()))+
            (previous.isEmpty()?"":"\n"+previous);
        new AlertDialog.Builder(activity).setTitle(kind.equals("game")?"Обновление игры":"Обновление WolfStudio").setMessage(text)
            .setPositiveButton("Проверить",(d,w)->{
                if(prefs(activity).getString("github_token","").isEmpty())askToken(activity,kind);else check(activity,kind);
            })
            .setNeutralButton("Ключ GitHub",(d,w)->askToken(activity,kind))
            .setNegativeButton("Ещё",(d,w)->new AlertDialog.Builder(activity).setItems(new String[]{"Вернуть прошлую версию","Закрыть"},(dd,which)->{
                if(which==0)new AlertDialog.Builder(activity).setTitle("Откат обновления").setMessage("После перезапуска будет восстановлена предыдущая версия. Миры и настройки останутся.").setPositiveButton("Вернуть",(ddd,www)->{rollback(activity,kind,"Откат выполнен пользователем");restart(activity);}).setNegativeButton("Отмена",null).show();
            }).show()).show();
    });}
    private static void askToken(Activity a,String kind){EditText field=new EditText(a);field.setSingleLine(true);field.setInputType(InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_VARIATION_PASSWORD);field.setText(prefs(a).getString("github_token",""));
        new AlertDialog.Builder(a).setTitle("Доступ к обновлениям GitHub").setMessage("Для закрытого dozyFJ/WolfCraft нужен fine-grained токен с правом Contents: Read-only только для этого репозитория. Он хранится в приватных данных приложения. Для открытого репозитория оставь поле пустым.").setView(field)
            .setPositiveButton("Проверить",(d,w)->{prefs(a).edit().putString("github_token",field.getText().toString().trim()).apply();check(a,kind);}).setNegativeButton("Отмена",null).show();
    }
    public static synchronized String loadedBuild(Context c,String kind,String part){String b=selectedBuild.get(kind+":"+part);return b==null||b.equals(BUILTIN)?metadata(c,"wolf.ota.build",BUILTIN):b;}
    private static String currentBuild(Context c,String kind){
        String build=active(c,kind).optString("build",BUILTIN);
        if(!build.equals(BUILTIN))try{JSONObject m=new JSONObject(new String(read(new File(new File(new File(root(c),kind),build),"manifest.json"),65536),StandardCharsets.UTF_8));if(m.getLong("sequence")>shellSequence(c))return build;}catch(Exception ignored){}
        return metadata(c,"wolf.ota.build",BUILTIN);
    }
    private static int saveFormat(Context c){return Integer.parseInt(metadata(c,"wolf.ota.saveformat","f3").replaceFirst("^f",""));}
    private static long shellSequence(Context c){try{return Long.parseLong(metadata(c,"wolf.ota.sequence","s0").replaceFirst("^s",""));}catch(Exception e){return 0;}}
    private static boolean allowedHost(String h){return Arrays.asList("api.github.com","github.com","release-assets.githubusercontent.com","objects.githubusercontent.com","github-releases.githubusercontent.com").contains(h);}
    private static HttpURLConnection request(String address,String token,String accept)throws Exception {
        URL url=new URL(address);
        for(int redirects=0;redirects<6;redirects++){
            if(!"https".equals(url.getProtocol())||!allowedHost(url.getHost()))throw new SecurityException("Недопустимый адрес обновления");
            HttpURLConnection c=(HttpURLConnection)url.openConnection();c.setInstanceFollowRedirects(false);c.setConnectTimeout(15000);c.setReadTimeout(30000);c.setRequestProperty("User-Agent","WolfCraft-OTA/1");c.setRequestProperty("Accept",accept);
            if(url.getHost().equals("api.github.com")){c.setRequestProperty("X-GitHub-Api-Version","2022-11-28");if(!token.isEmpty())c.setRequestProperty("Authorization","Bearer "+token);}
            int status=c.getResponseCode();
            if(status==301||status==302||status==303||status==307||status==308){String next=c.getHeaderField("Location");c.disconnect();if(next==null)throw new IOException("Нет адреса архива");url=new URL(url,next);continue;}
            if(status!=200){c.disconnect();throw new IOException(status==401||status==403||status==404?"GitHub не дал доступ к обновлению ("+status+"). Проверь ключ и права чтения репозитория.":"Ошибка GitHub: "+status);}
            return c;
        }throw new IOException("Слишком много переадресаций");
    }
    private static byte[] get(String address,String token,int limit)throws Exception{HttpURLConnection c=request(address,token,"application/octet-stream");try(InputStream in=c.getInputStream()){return readLimited(in,limit);}finally{c.disconnect();}}
    private static JSONObject asset(JSONObject release,String name)throws Exception{JSONArray a=release.getJSONArray("assets");for(int i=0;i<a.length();i++){JSONObject v=a.getJSONObject(i);if(name.equals(v.getString("name")))return v;}throw new IOException("В релизе нет "+name);}
    private static void status(Activity a,ProgressDialog p,String text){a.runOnUiThread(()->{if(!a.isFinishing())p.setMessage(text);});}
    private static void check(Activity a,String kind){
        if(!busy.compareAndSet(false,true))return;
        ProgressDialog progress=new ProgressDialog(a);progress.setTitle(kind.equals("game")?"Обновление игры":"Обновление WolfStudio");progress.setMessage("Проверка GitHub…");progress.setCancelable(false);progress.show();
        new Thread(()->{
            File archive=null,stage=null;String outcome=null;boolean installed=false;
            try(RandomAccessFile lockFile=new RandomAccessFile(new File(root(a),"update.lock"),"rw");FileChannel channel=lockFile.getChannel();FileLock lock=channel.tryLock()){
                if(lock==null)throw new IOException("Обновление уже выполняется в другом окне");
                String token=prefs(a).getString("github_token","");
                String tag=kind.equals("game")?"game-ota":"studio-ota";
                HttpURLConnection conn=request(REPO+"/releases/tags/"+tag,token,"application/vnd.github+json");JSONObject release;
                try(InputStream in=conn.getInputStream()){release=new JSONObject(new String(readLimited(in,1024*1024),StandardCharsets.UTF_8));}finally{conn.disconnect();}
                byte[] raw=get(asset(release,kind+"-ota.json").getString("url"),token,65536);
                byte[] signature=get(asset(release,kind+"-ota.sig").getString("url"),token,8192);
                JSONObject manifest=BundleVerifier.manifest(raw,signature,publicKey(a),kind);String build=manifest.getString("build");
                if(manifest.getInt("save_format")!=saveFormat(a))throw new SecurityException("Изменился формат сохранений. Нужен новый APK; текущая версия оставлена без изменений.");
                if(!manifest.getString("shell_hash").equals(metadata(a,"wolf.ota.shellhash","development")))throw new SecurityException("Обновилась Android-оболочка. Для этой версии нужен новый APK-загрузчик; текущая версия оставлена без изменений.");
                if(build.equals(currentBuild(a,kind)))outcome="Уже установлена актуальная версия.";
                else{
                    long currentSequence=shellSequence(a);JSONObject old=active(a,kind);String oldBuild=old.optString("build",BUILTIN);
                    if(!oldBuild.equals(BUILTIN)){try{currentSequence=Math.max(currentSequence,verify(a,kind,new File(new File(root(a),kind),oldBuild)).getLong("sequence"));}catch(Exception ignored){}}
                    if(manifest.getLong("sequence")<=currentSequence)throw new SecurityException("Доступная версия не новее установленной. Понижение через обновление запрещено.");
                    JSONObject zip=asset(release,manifest.getString("archive"));long zipSize=zip.getLong("size");if(zipSize<=0||zipSize>BundleVerifier.MAX_BYTES)throw new SecurityException("Недопустимый размер архива");
                    archive=File.createTempFile("wolf-ota-",".zip",a.getCacheDir());HttpURLConnection download=request(zip.getString("url"),token,"application/octet-stream");
                    try(InputStream in=download.getInputStream();FileOutputStream out=new FileOutputStream(archive)){byte[] b=new byte[65536];long received=0,deadline=SystemClock.elapsedRealtime()+180000;int n,last=-1;while((n=in.read(b))!=-1){if(SystemClock.elapsedRealtime()>deadline)throw new IOException("Превышено время загрузки");received+=n;if(received>zipSize||received>BundleVerifier.MAX_BYTES)throw new SecurityException("Архив больше ожидаемого");out.write(b,0,n);int pct=(int)(received*100/zipSize);if(pct!=last){status(a,progress,"Загрузка обновления: "+pct+" %");last=pct;}}if(received!=zipSize)throw new IOException("Загрузка прервалась");out.getFD().sync();}finally{download.disconnect();}
                    status(a,progress,"Проверка подписи и библиотек…");
                    File versions=new File(root(a),kind);versions.mkdirs();stage=new File(versions,"staging-"+UUID.randomUUID());BundleVerifier.extract(archive,stage,manifest);
                    write(new File(stage,"manifest.json"),raw);write(new File(stage,"manifest.sig"),signature);
                    File ready=new File(versions,build);
                    if(ready.exists()){verify(a,kind,ready);BundleVerifier.delete(stage);stage=null;}else{if(!stage.renameTo(ready))throw new IOException("Не удалось завершить обновление");stage=null;}
                    JSONObject next=new JSONObject().put("build",build).put("previous",old.optString("build",BUILTIN));write(pointer(a,kind),next.toString().getBytes(StandardCharsets.UTF_8));
                    for(String part:new String[]{"native","server"})new AtomicFile(pending(a,kind,part)).delete();
                    prefs(a).edit().remove("last_error").apply();installed=true;outcome="Приложение обновлено до "+build.substring(0,10)+". Перезапусти приложение, чтобы начать играть на новой версии.";
                }
            }catch(Exception e){outcome=e.getMessage()==null?e.getClass().getSimpleName():e.getMessage();}
            finally{if(archive!=null)archive.delete();if(stage!=null)BundleVerifier.delete(stage);busy.set(false);}
            String message=outcome;boolean restart=installed;
            a.runOnUiThread(()->{try{progress.dismiss();if(!a.isFinishing()){AlertDialog.Builder dialog=new AlertDialog.Builder(a).setTitle(restart?"Обновление готово":kind.equals("game")?"Обновление игры":"Обновление WolfStudio").setMessage(message);if(restart)dialog.setPositiveButton("Перезапустить",(d,w)->restart(a)).setNegativeButton("Позже",null);else dialog.setPositiveButton("Понятно",null);dialog.show();}}catch(Exception ignored){}});
        },"wolf-ota").start();
    }
    public static void restart(Activity a){
        new Thread(()->{
            // Give the native server a chance to persist stop hooks before killing its process.
            String token=prefs(a).getString("server_token","");
            if(!token.isEmpty())try{HttpURLConnection c=(HttpURLConnection)new URL("http://127.0.0.1:25580/api/stop?t="+URLEncoder.encode(token,"UTF-8")).openConnection();c.setRequestMethod("POST");c.setConnectTimeout(1000);c.setReadTimeout(2500);c.getInputStream().close();c.disconnect();}catch(Exception ignored){}
            a.runOnUiThread(()->{a.startActivity(new Intent(a,RestartActivity.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));});
        },"wolf-restart").start();
    }
}
