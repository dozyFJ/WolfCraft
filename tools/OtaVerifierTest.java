import com.wolfcraft.ota.BundleVerifier;
import java.io.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;
import java.util.zip.*;
import org.json.*;

/** JVM checks use synthetic ELF headers; actual ABI exports are checked by build_ota.py. */
public final class OtaVerifierTest {
    static int checks;
    static byte[] elf(){byte[] b=new byte[128];b[0]=0x7f;b[1]='E';b[2]='L';b[3]='F';b[4]=2;b[5]=1;b[16]=3;b[18]=(byte)183;return b;}
    static File zip(File root,String name,byte[] bytes)throws Exception{File f=new File(root,UUID.randomUUID()+".zip");try(ZipOutputStream z=new ZipOutputStream(new FileOutputStream(f))){z.putNextEntry(new ZipEntry(name));z.write(bytes);z.closeEntry();}return f;}
    static JSONObject manifest(File zip,byte[] elf)throws Exception{
        File temp=File.createTempFile("hash-",".so");Files.write(temp.toPath(),elf);String hash=BundleVerifier.sha256(temp);temp.delete();
        return new JSONObject().put("format",1).put("shell_api",1).put("shell_hash","a".repeat(64)).put("save_format",3).put("kind","game").put("abi","arm64-v8a").put("build","1".repeat(40)).put("sequence",100)
          .put("archive","game-runtime.zip").put("sha256",BundleVerifier.sha256(zip)).put("files",new JSONArray().put(new JSONObject().put("name","libwolfcraft_game.so").put("size",elf.length).put("sha256",hash)));
    }
    static byte[] sign(byte[] bytes,PrivateKey key)throws Exception{Signature s=Signature.getInstance("SHA256withRSA");s.initSign(key);s.update(bytes);return s.sign();}
    interface Action{void run()throws Exception;}
    static void rejects(Action test)throws Exception{try{test.run();throw new AssertionError("accepted invalid update");}catch(SecurityException|org.json.JSONException expected){checks++;}}
    static JSONObject copy(JSONObject o){return new JSONObject(o.toString());}
    public static void main(String[] args)throws Exception{
        File root=Files.createTempDirectory("wolf-ota-tests").toFile();
        try{
            KeyPairGenerator g=KeyPairGenerator.getInstance("RSA");g.initialize(2048);KeyPair keys=g.generateKeyPair();byte[] elf=elf();File archive=zip(root,"libwolfcraft_game.so",elf);JSONObject m=manifest(archive,elf);byte[] raw=m.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8);byte[] sig=sign(raw,keys.getPrivate());
            JSONObject verified=BundleVerifier.manifest(raw,sig,keys.getPublic(),"game");assert verified.getString("build").equals("1".repeat(40));checks++;
            File installed=new File(root,"installed");BundleVerifier.extract(archive,installed,verified);BundleVerifier.verifyDirectory(installed,verified);assert new File(installed,"libwolfcraft_game.so").length()==128;checks++;

            for(String kind:new String[]{"game","studio"}){
                String[] names=kind.equals("game")?new String[]{"libwolfcraft_game.so","libc++_shared.so"}:new String[]{"libwolfstudio.so","libwolfserver_runtime.so","libc++_shared.so"};
                File bundle=new File(root,kind+"-with-cpp.zip");
                JSONArray files=new JSONArray();
                try(ZipOutputStream z=new ZipOutputStream(new FileOutputStream(bundle))){
                    for(String name:names){
                        z.putNextEntry(new ZipEntry(name));z.write(elf);z.closeEntry();
                        files.put(new JSONObject().put("name",name).put("size",elf.length).put("sha256",m.getJSONArray("files").getJSONObject(0).getString("sha256")));
                    }
                }
                JSONObject cpp=manifest(bundle,elf).put("kind",kind).put("archive",kind+"-runtime.zip").put("files",files);
                byte[] cr=cpp.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8);
                JSONObject cv=BundleVerifier.manifest(cr,sign(cr,keys.getPrivate()),keys.getPublic(),kind);
                File target=new File(root,kind+"-with-cpp");
                BundleVerifier.extract(bundle,target,cv);BundleVerifier.verifyDirectory(target,cv);
                assert new File(target,"libc++_shared.so").length()==elf.length;checks++;
            }
            for(String name:new String[]{"libevil++.so","../libc++_shared.so","libc++_shared.so/evil","libc++_shared.so.so"}){
                JSONObject bad=copy(m);bad.getJSONArray("files").put(new JSONObject(m.getJSONArray("files").getJSONObject(0).toString()).put("name",name));
                byte[] b=bad.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8);
                rejects(()->BundleVerifier.manifest(b,sign(b,keys.getPrivate()),keys.getPublic(),"game"));
            }
            byte[] wrong=sig.clone();wrong[0]^=1;rejects(()->BundleVerifier.manifest(raw,wrong,keys.getPublic(),"game"));
            rejects(()->BundleVerifier.manifest(raw,sig,g.generateKeyPair().getPublic(),"game"));
            rejects(()->BundleVerifier.manifest(raw,sig,keys.getPublic(),"studio"));
            for(String field:new String[]{"abi","build","shell_hash","archive","sha256"}){
                JSONObject bad=copy(m).put(field,"../bad");byte[] b=bad.toString().getBytes();rejects(()->BundleVerifier.manifest(b,sign(b,keys.getPrivate()),keys.getPublic(),"game"));
            }
            JSONObject api=copy(m).put("shell_api",2);byte[] bytes=api.toString().getBytes();rejects(()->BundleVerifier.manifest(bytes,sign(bytes,keys.getPrivate()),keys.getPublic(),"game"));
            JSONObject large=copy(m);large.getJSONArray("files").getJSONObject(0).put("size",BundleVerifier.MAX_BYTES+1);byte[] lb=large.toString().getBytes();rejects(()->BundleVerifier.manifest(lb,sign(lb,keys.getPrivate()),keys.getPublic(),"game"));
            JSONObject path=copy(m);path.getJSONArray("files").getJSONObject(0).put("name","../outside.so");byte[] pb=path.toString().getBytes();rejects(()->BundleVerifier.manifest(pb,sign(pb,keys.getPrivate()),keys.getPublic(),"game"));
            File traversal=zip(root,"../outside.so",elf);JSONObject tm=manifest(traversal,elf);rejects(()->BundleVerifier.extract(traversal,new File(root,"traversal"),tm));assert !new File(root,"outside.so").exists();checks++;
            File missing=zip(root,"libother.so",elf);JSONObject mm=manifest(missing,elf);rejects(()->BundleVerifier.extract(missing,new File(root,"missing"),mm));
            File shortZip=zip(root,"libwolfcraft_game.so",new byte[1]);JSONObject sm=manifest(shortZip,elf);rejects(()->BundleVerifier.extract(shortZip,new File(root,"short"),sm));
            File badHash=zip(root,"libwolfcraft_game.so",elf);JSONObject hm=manifest(badHash,elf);Files.write(badHash.toPath(),new byte[]{1,2,3});rejects(()->BundleVerifier.extract(badHash,new File(root,"hash"),hm));
            byte[] badElf=elf.clone();badElf[18]=0;File archZip=zip(root,"libwolfcraft_game.so",badElf);JSONObject am=manifest(archZip,badElf);rejects(()->BundleVerifier.extract(archZip,new File(root,"architecture"),am));
            new File(installed,"libwolfcraft_game.so").setWritable(true);Files.write(new File(installed,"libwolfcraft_game.so").toPath(),badElf);rejects(()->BundleVerifier.verifyDirectory(installed,m));
            System.out.println("OTA verifier: "+checks+" checks passed");
        }finally{BundleVerifier.delete(root);}
    }
}
