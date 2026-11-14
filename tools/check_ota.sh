#!/usr/bin/env bash
set -euo pipefail
ota_test_dir="$(mktemp -d)"
trap 'rm -rf "$ota_test_dir"' EXIT
curl -fsSL --retry 3 https://repo.maven.apache.org/maven2/org/json/json/20240303/json-20240303.jar -o "$ota_test_dir/json.jar"
echo "3cf6cd6892e32e2b4c1c39e0f52f5248a2f5b37646fdfbb79a66b46b618414ed  $ota_test_dir/json.jar" | sha256sum -c -
keytool -exportcert -alias androiddebugkey -keystore ci/wolfcraft-release.keystore -storepass android -file "$ota_test_dir/cert.der" >/dev/null
javac --release 17 -cp "$ota_test_dir/json.jar" -d "$ota_test_dir/classes" android-shared/src/main/java/com/wolfcraft/ota/BundleVerifier.java tools/OtaBundleSmokeTest.java
java -ea -cp "$ota_test_dir/classes:$ota_test_dir/json.jar" OtaBundleSmokeTest "$1" "$2" "$ota_test_dir/cert.der"
