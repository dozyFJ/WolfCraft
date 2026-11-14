import importlib.util,json,pathlib,struct,subprocess,tempfile,unittest,zipfile
spec=importlib.util.spec_from_file_location('build_ota',pathlib.Path(__file__).with_name('build_ota.py'))
ota=importlib.util.module_from_spec(spec);spec.loader.exec_module(ota)
class ProducerTest(unittest.TestCase):
 def test_signed_bundle_matches_manifest_and_rejects_invalid_native_libraries(self):
  with tempfile.TemporaryDirectory() as tmp:
   d=pathlib.Path(tmp);libs=d/'libs';libs.mkdir();out=d/'out';key=d/'signing.p12'
   subprocess.run(['keytool','-genkeypair','-alias','androiddebugkey','-keyalg','RSA','-keysize','2048','-validity','1','-dname','CN=Wolf OTA Test','-keystore',str(key),'-storepass','android','-keypass','android','-storetype','PKCS12','-noprompt'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
   data=bytearray(128);data[:6]=b'\x7fELF\x02\x01';struct.pack_into('<HH',data,16,3,183)
   (libs/'libwolfcraft_game.so').write_bytes(data)
   nm=d/'nm';nm.write_text('#!/bin/sh\nprintf "000000 T ANativeActivity_onCreate\\n"\n');nm.chmod(0o755)
   m=ota.build('game',libs,out,'a'*40,200,key,str(nm),'b'*64)
   self.assertEqual(m['files'][0]['sha256'],ota.digest(data));self.assertEqual(m['sequence'],200)
   with zipfile.ZipFile(out/'game-runtime.zip') as z:self.assertEqual(z.namelist(),['libwolfcraft_game.so']);self.assertEqual(z.read('libwolfcraft_game.so'),data)
   subprocess.run(['keytool','-exportcert','-rfc','-keystore',str(key),'-storepass','android','-alias','androiddebugkey','-file',str(d/'cert.pem')],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
   subprocess.run(['openssl','x509','-in',str(d/'cert.pem'),'-pubkey','-noout','-out',str(d/'public.pem')],check=True)
   subprocess.run(['openssl','dgst','-sha256','-verify',str(d/'public.pem'),'-signature',str(out/'game-ota.sig'),str(out/'game-ota.json')],check=True,stdout=subprocess.DEVNULL)
   nm.write_text('#!/bin/sh\nprintf "000000 T wrong_entry\\n"\n')
   with self.assertRaisesRegex(ValueError,'Missing shell ABI'):ota.build('game',libs,out,'a'*40,201,key,str(nm),'b'*64)
   data[18]=0;(libs/'libwolfcraft_game.so').write_bytes(data)
   with self.assertRaisesRegex(ValueError,'AArch64'):ota.build('game',libs,out,'a'*40,201,key,str(nm),'b'*64)
if __name__=='__main__':unittest.main()
