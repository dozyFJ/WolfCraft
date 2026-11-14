import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

val runNumber = System.getenv("GITHUB_RUN_NUMBER")?.toIntOrNull() ?: 1

android {
    namespace = "com.wolfcraft.server"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.wolfcraft.server"
        minSdk = 26
        targetSdk = 33
        versionCode = 200000 + runNumber
        versionName = "0.1.$runNumber"
        manifestPlaceholders["otaSaveFormat"] = System.getenv("OTA_SAVE_FORMAT") ?: "3"
        manifestPlaceholders["otaShellHash"] = System.getenv("OTA_SHELL_HASH") ?: "development"
        manifestPlaceholders["otaBuild"] = System.getenv("GITHUB_SHA") ?: "builtin"
        manifestPlaceholders["otaSequence"] = System.getenv("GITHUB_RUN_ID") ?: "0"
        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    sourceSets.getByName("main").java.srcDir("../../android-shared/src/main/java")

    // Same key as the game's CI builds (ci/wolfcraft-release.keystore), so updates install
    // over the previous version.
    signingConfigs {
        create("release") {
            storeFile = file("../../ci/wolfcraft-release.keystore")
            storePassword = "android"
            keyAlias = "androiddebugkey"
            keyPassword = "android"
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("release")
        }
        debug {
            signingConfig = signingConfigs.getByName("release")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // Initial JNI cores are extracted from the APK; signed OTA cores are loaded from
    // app-private storage. Downloaded executables are never started with exec().
    packaging {
        jniLibs {
            useLegacyPackaging = true
        }
    }

    lint {
        abortOnError = false
        checkReleaseBuilds = false
    }
}

