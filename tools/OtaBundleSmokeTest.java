import com.wolfcraft.ota.BundleVerifier;
import java.io.*;
import java.nio.file.*;
import java.security.PublicKey;
import java.security.cert.CertificateFactory;
import org.json.JSONObject;

/** Validate the actual signed release archive using the same verifier shipped in the APK. */
public final class OtaBundleSmokeTest {
    public static void main(String[] args) throws Exception {
        String kind=args[0];Path bundle=Path.of(args[1]);PublicKey key;
        try(InputStream in=Files.newInputStream(Path.of(args[2]))){
            key=CertificateFactory.getInstance("X.509").generateCertificate(in).getPublicKey();
        }
        JSONObject manifest=BundleVerifier.manifest(Files.readAllBytes(bundle.resolve(kind+"-ota.json")),
            Files.readAllBytes(bundle.resolve(kind+"-ota.sig")),key,kind);
        File extracted=Files.createTempDirectory("wolf-release-check-").toFile();
        try{
            BundleVerifier.extract(bundle.resolve(kind+"-runtime.zip").toFile(),extracted,manifest);
            BundleVerifier.verifyDirectory(extracted,manifest);
            if(!new File(extracted,"libc++_shared.so").isFile())throw new AssertionError("NDK runtime missing from release");
            System.out.println("Actual "+kind+" OTA: signature, manifest and "+manifest.getJSONArray("files").length()+" native libraries verified");
        }finally{BundleVerifier.delete(extracted);}
    }
}
