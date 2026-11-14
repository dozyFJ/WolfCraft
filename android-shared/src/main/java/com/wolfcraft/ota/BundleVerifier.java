package com.wolfcraft.ota;

import java.io.*;
import java.nio.file.Files;
import java.security.*;
import java.util.*;
import java.util.zip.*;
import org.json.*;

/** Pure Java validation, also used by JVM tests. No archive path controls an output path. */
public final class BundleVerifier {
    public static final long MAX_BYTES=160L*1024*1024;
    public static final int SHELL_API=1;
    public static String sha256(File file) throws Exception {
        MessageDigest hash=MessageDigest.getInstance("SHA-256");
        try(InputStream in=new FileInputStream(file)){byte[] b=new byte[65536];int n;while((n=in.read(b))!=-1)hash.update(b,0,n);}
        StringBuilder out=new StringBuilder();for(byte b:hash.digest())out.append(String.format(java.util.Locale.ROOT,"%02x",b&255));return out.toString();
    }
    public static JSONObject manifest(byte[] bytes,byte[] signature,PublicKey key,String kind) throws Exception {
        if(bytes.length>65536||signature.length>8192)throw new SecurityException("Слишком большой манифест");
        Signature verifier=Signature.getInstance("SHA256withRSA");verifier.initVerify(key);verifier.update(bytes);
        if(!verifier.verify(signature))throw new SecurityException("Подпись обновления не совпадает с подписью приложения");
        JSONObject m=new JSONObject(new String(bytes,java.nio.charset.StandardCharsets.UTF_8));
        if(m.getInt("format")!=1||!kind.equals(m.getString("kind"))||!"arm64-v8a".equals(m.getString("abi")))throw new SecurityException("Обновление для другого приложения или процессора");
        if(m.getInt("shell_api")!=SHELL_API)throw new SecurityException("Нужен новый APK-загрузчик: изменился Android API оболочки");
        if(m.getInt("save_format")<1||m.getInt("save_format")>65535)throw new SecurityException("Неверный формат сохранений");
        if(!m.getString("shell_hash").matches("[0-9a-f]{64}"))throw new SecurityException("Неверная версия оболочки");
        if(!m.getString("build").matches("[0-9a-f]{40}"))throw new SecurityException("Неверная версия");
        if(!m.getString("archive").equals(kind+"-runtime.zip")||!m.getString("sha256").matches("[0-9a-f]{64}"))throw new SecurityException("Неверный архив");
        JSONArray files=m.getJSONArray("files");if(files.length()<1||files.length()>8)throw new SecurityException("Неверное число библиотек");
        Set<String> names=new HashSet<>();long total=0;
        for(int i=0;i<files.length();i++){JSONObject f=files.getJSONObject(i);String name=f.getString("name");
            if(!name.matches("lib[a-zA-Z0-9_]+\\.so")||!names.add(name)||!f.getString("sha256").matches("[0-9a-f]{64}"))throw new SecurityException("Неверное имя библиотеки");
            long size=f.getLong("size");if(size<20||size>MAX_BYTES)throw new SecurityException("Неверный размер библиотеки");total+=size;
        }
        if(total>MAX_BYTES)throw new SecurityException("Обновление слишком большое");
        if(!names.contains(kind.equals("game")?"libwolfcraft_game.so":"libwolfstudio.so"))throw new SecurityException("Нет ядра приложения");
        if(kind.equals("studio")&&!names.contains("libwolfserver_runtime.so"))throw new SecurityException("Нет ядра сервера");
        return m;
    }
    public static void verifyDirectory(File dir,JSONObject manifest) throws Exception {
        JSONArray files=manifest.getJSONArray("files");
        for(int i=0;i<files.length();i++){JSONObject meta=files.getJSONObject(i);File file=new File(dir,meta.getString("name"));
            if(!file.getCanonicalFile().getParentFile().equals(dir.getCanonicalFile())||file.length()!=meta.getLong("size")||!sha256(file).equals(meta.getString("sha256")))throw new SecurityException("Повреждена библиотека "+meta.getString("name"));
            // ELF64 little-endian AArch64 shared object, never an arbitrary executable.
            try(InputStream in=new FileInputStream(file)){byte[] h=new byte[20];if(in.read(h)!=20||h[0]!=0x7f||h[1]!='E'||h[2]!='L'||h[3]!='F'||h[4]!=2||h[5]!=1||h[16]!=3||h[17]!=0||(h[18]&255)!=183||h[19]!=0)throw new SecurityException("Неверный формат ядра");}
        }
    }
    public static void extract(File archive,File dir,JSONObject manifest) throws Exception {
        if(!sha256(archive).equals(manifest.getString("sha256")))throw new SecurityException("Контрольная сумма архива не совпала");
        if(dir.exists()&&!Arrays.asList(dir.list()).isEmpty())throw new SecurityException("Папка обновления не пуста");
        if(!dir.isDirectory()&&!dir.mkdirs())throw new IOException("Не удалось создать папку обновления");
        Map<String,JSONObject> expected=new HashMap<>();JSONArray files=manifest.getJSONArray("files");for(int i=0;i<files.length();i++){JSONObject f=files.getJSONObject(i);expected.put(f.getString("name"),f);}
        Set<String> seen=new HashSet<>();long total=0;
        try(ZipInputStream zip=new ZipInputStream(new FileInputStream(archive))){ZipEntry entry;byte[] b=new byte[65536];
            while((entry=zip.getNextEntry())!=null){JSONObject meta=expected.get(entry.getName());if(entry.isDirectory()||meta==null||!seen.add(entry.getName()))throw new SecurityException("Лишний или повторяющийся файл в архиве");
                File file=new File(dir,entry.getName());long count=0;
                // Open before making the code read-only; a completed update is immutable.
                try(FileOutputStream out=new FileOutputStream(file)){if(!file.setReadOnly())throw new IOException("Нельзя защитить библиотеку от записи");int n;while((n=zip.read(b))!=-1){count+=n;total+=n;if(count>meta.getLong("size")||total>MAX_BYTES)throw new SecurityException("Превышен размер обновления");out.write(b,0,n);}out.getFD().sync();}
                if(count!=meta.getLong("size"))throw new SecurityException("Неполная библиотека");zip.closeEntry();
            }
        }
        if(seen.size()!=expected.size())throw new SecurityException("Неполное обновление");verifyDirectory(dir,manifest);
    }
    public static void delete(File file){if(file.isDirectory()){File[] children=file.listFiles();if(children!=null)for(File c:children)delete(c);}file.delete();}
}
