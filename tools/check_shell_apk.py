#!/usr/bin/env python3
import pathlib,sys,zipfile
kind,apk=sys.argv[1:]
required=['libwolfcraft_game.so'] if kind=='game' else ['libwolfstudio.so','libwolfserver_runtime.so']
with zipfile.ZipFile(apk) as z:
 names=set(z.namelist())
 for name in required:
  if 'lib/arm64-v8a/'+name not in names:raise SystemExit('APK missing '+name)
 dex=b''.join(z.read(n) for n in names if n.endswith('.dex'))
 for cls in ['NativeHostActivity','OtaManager','RestartActivity','BundleVerifier']:
  if ('com/wolfcraft/ota/'+cls).encode() not in dex:raise SystemExit('APK missing Java shell '+cls)
 if kind=='studio' and 'lib/arm64-v8a/libwolfserver.so' in names:raise SystemExit('Legacy executable is still packaged')
print('APK shell and native cores verified:',kind)
