plugins { id("com.android.application") }
val runNumber = System.getenv("GITHUB_RUN_NUMBER")?.toIntOrNull() ?: 1
android {
    namespace = "com.wolfcraft.mobile"
    compileSdk = 35
    defaultConfig {
        applicationId = "com.wolfcraft.mobile"
        minSdk = 26
        targetSdk = 35
        versionCode = 200000 + runNumber
        versionName = "0.3.ota.$runNumber"
        ndk { abiFilters += listOf("arm64-v8a") }
        manifestPlaceholders["otaSaveFormat"] = System.getenv("OTA_SAVE_FORMAT") ?: "3"
        manifestPlaceholders["otaShellHash"] = System.getenv("OTA_SHELL_HASH") ?: "development"
        manifestPlaceholders["otaBuild"] = System.getenv("GITHUB_SHA") ?: "builtin"
        manifestPlaceholders["otaSequence"] = System.getenv("GITHUB_RUN_ID") ?: "0"
    }
    sourceSets.getByName("main").java.srcDir("../../android-shared/src/main/java")
    signingConfigs { create("release") {
        storeFile = file("../../ci/wolfcraft-release.keystore")
        storePassword = "android"; keyAlias = "androiddebugkey"; keyPassword = "android"
    } }
    buildTypes { release { isMinifyEnabled = false; signingConfig = signingConfigs.getByName("release") } }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
    packaging { jniLibs { useLegacyPackaging = true } }
    lint { abortOnError = false; checkReleaseBuilds = false }
}
