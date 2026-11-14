#!/usr/bin/env python3
"""Fingerprint only the Android shell. Native Rust changes do not change this hash."""
import hashlib,pathlib,sys

def shell_hash(kind,root=pathlib.Path('.')):
    patterns=['android-shared/src/main/java/**/*.java']
    if kind=='game':patterns+=['game-pocket/app/src/main/AndroidManifest.xml','game-pocket/app/build.gradle.kts','game-pocket/app/src/main/res/**/*']
    elif kind=='studio':patterns+=['wolfserver-pocket/app/src/main/**/*.kt','wolfserver-pocket/app/src/main/AndroidManifest.xml','wolfserver-pocket/app/build.gradle.kts','wolfserver-pocket/app/src/main/res/**/*']
    else:raise ValueError('Invalid kind')
    files=sorted({p for pattern in patterns for p in root.glob(pattern) if p.is_file()})
    if not files:raise ValueError('No shell source files')
    h=hashlib.sha256()
    for p in files:h.update(str(p.relative_to(root)).encode()+b'\0'+p.read_bytes()+b'\0')
    return h.hexdigest()
if __name__=='__main__':print(shell_hash(sys.argv[1]))
