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
        versionCode = runNumber
        versionName = "0.1.$runNumber"
        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

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

    // The server binary ships as lib/arm64-v8a/libwolfserver.so and must be unpacked to the
    // native library folder: that is the one place Android lets an app start a program from.
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
