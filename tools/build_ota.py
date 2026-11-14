#!/usr/bin/env python3
"""Build and sign an immutable native OTA bundle with the APK's existing RSA signing key."""
import argparse, hashlib, json, os, pathlib, re, shutil, struct, subprocess, tempfile, zipfile

def digest(data): return hashlib.sha256(data).hexdigest()
def build(kind, source, output, revision, sequence, keystore, nm='nm', shell_hash=''):
    if len(shell_hash)!=64 or any(c not in '0123456789abcdef' for c in shell_hash): raise ValueError('Invalid shell fingerprint')
    if kind not in ('game','studio') or len(revision)!=40 or any(c not in '0123456789abcdef' for c in revision):
        raise ValueError('Invalid kind or commit')
    output.mkdir(parents=True,exist_ok=True)
    names=sorted(p.name for p in source.glob('*.so'))
    required={'libwolfcraft_game.so'} if kind=='game' else {'libwolfstudio.so','libwolfserver_runtime.so'}
    if not required.issubset(names): raise ValueError(f'Missing native libraries: {required-set(names)}')
    if len(names)>8: raise ValueError('Too many libraries')
    exports={
      'libwolfcraft_game.so':['ANativeActivity_onCreate'],
      'libwolfstudio.so':['ANativeActivity_onCreate'],
      'libwolfserver_runtime.so':['Java_com_wolfcraft_server_ServerService_nativeRun','Java_com_wolfcraft_server_ServerService_nativeStop'],
    }
    entries=[]
    for name in names:
        data=(source/name).read_bytes()
        if len(data)<20 or data[:6]!=b'\x7fELF\x02\x01' or struct.unpack_from('<HH',data,16)!=(3,183):
            raise ValueError(f'{name} is not an AArch64 shared library')
        if name in exports:
            symbols=subprocess.check_output([nm,'-D','--defined-only',str(source/name)],text=True)
            for symbol in exports[name]:
                if not any(line.split()[-1]==symbol for line in symbols.splitlines() if line.split()):
                    raise ValueError(f'Missing shell ABI symbol {symbol} in {name}')
        entries.append({'name':name,'size':len(data),'sha256':digest(data)})
    if sum(x['size'] for x in entries)>160*1024*1024: raise ValueError('Bundle exceeds size limit')
    archive=output/f'{kind}-runtime.zip'
    with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
        for e in entries:
            info=zipfile.ZipInfo(e['name'],(2026,1,1,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;info.external_attr=0o444<<16
            z.writestr(info,(source/e['name']).read_bytes())
    version=(pathlib.Path(__file__).resolve().parents[1]/'wolfcraft-common/src/version.rs').read_text()
    save_format=int(re.search(r'pub const SAVE_FORMAT\s*:\s*u32\s*=\s*(\d+)',version).group(1))
    manifest={'save_format':save_format,'format':1,'shell_api':1,'shell_hash':shell_hash,'kind':kind,'abi':'arm64-v8a','build':revision,'sequence':int(sequence),'archive':archive.name,'sha256':digest(archive.read_bytes()),'files':entries}
    raw=json.dumps(manifest,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()+b'\n'
    manifest_path=output/f'{kind}-ota.json';manifest_path.write_bytes(raw)
    with tempfile.TemporaryDirectory(prefix='wolf-ota-sign-') as tmp:
        p=pathlib.Path(tmp);private=p/'private.pem';pkcs=p/'key.p12'
        subprocess.run(['keytool','-importkeystore','-srckeystore',str(keystore),'-srcstorepass','android','-srckeypass','android','-srcalias','androiddebugkey','-destkeystore',str(pkcs),'-deststoretype','PKCS12','-deststorepass','android','-destkeypass','android','-noprompt'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        subprocess.run(['openssl','pkcs12','-in',str(pkcs),'-passin','pass:android','-nocerts','-nodes','-out',str(private)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        os.chmod(private,0o600)
        subprocess.run(['openssl','dgst','-sha256','-sign',str(private),'-out',str(output/f'{kind}-ota.sig'),str(manifest_path)],check=True)
    return manifest

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--kind',required=True,choices=['game','studio']);p.add_argument('--input',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--build',required=True);p.add_argument('--sequence',required=True);p.add_argument('--keystore',type=pathlib.Path,required=True);p.add_argument('--nm',default='nm');p.add_argument('--shell-hash',required=True)
    a=p.parse_args();m=build(a.kind,a.input,a.output,a.build,a.sequence,a.keystore,a.nm,a.shell_hash);print(f"Signed {a.kind} native OTA {m['build']} ({len(m['files'])} libraries)")
