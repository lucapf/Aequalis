import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

// The upload key: keystore.properties (not in git), or the same names in the environment (CI).
val keystore = Properties().apply {
    rootProject.file("keystore.properties").takeIf { it.exists() }?.inputStream()?.use { load(it) }
}
fun signing(name: String) = keystore.getProperty(name) ?: System.getenv("AEQUALIS_" + name.uppercase())
val hasUploadKey = signing("storeFile") != null

// 1.2.3 → 10203: the version code goes up with the version, as Play needs.
val appVersion = System.getenv("AEQUALIS_VERSION") ?: "1.0.0"
val appVersionCode = appVersion.split(".").map { it.toInt() }.let { (a, b, c) -> a * 10000 + b * 100 + c }

android {
    namespace = "io.lucapf.aequalis"
    compileSdk {
        version = release(37)
    }

    defaultConfig {
        applicationId = "io.lucapf.aequalis"
        minSdk = 26
        targetSdk = 37
        versionCode = appVersionCode
        versionName = appVersion

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    signingConfigs {
        if (hasUploadKey) {
            create("upload") {
                storeFile = file(signing("storeFile")!!)
                storePassword = signing("storePassword")
                keyAlias = signing("keyAlias")
                keyPassword = signing("keyPassword")
            }
        }
    }

    buildTypes {
        // The fast build, for using the app (scripts/install-android.sh): compiled ahead of time,
        // where the debug build runs in the interpreter so that a debugger can stop it anywhere.
        release {
            // The upload key when there is one (Play); otherwise the debug key, as the debug build:
            // either installs over the other, the app's data kept (scripts/install-android.sh).
            signingConfig = signingConfigs.getByName(if (hasUploadKey) "upload" else "debug")
            // R8 on the whole app. Its default rules keep the names of the native methods, which the
            // Rust core's functions are found by (`Java_io_lucapf_aequalis_Core_init`, src/jni.rs).
            // Not `packageScope` (androidx, kotlin): it moved a class out of kotlin.collections, away
            // from its parent class, and the app stopped at start (IllegalAccessError).
            optimization {
                enable = true
            }
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    buildFeatures {
        compose = true
    }
}

// The bundle is for Play: never signed with the debug key by mistake.
tasks.matching { it.name == "bundleRelease" }.configureEach {
    doFirst {
        check(hasUploadKey) { "bundleRelease needs the upload key: keystore.properties or AEQUALIS_STOREFILE (docs/Android-release.md)" }
    }
}

dependencies {
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    testImplementation(libs.junit)
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.compose.ui.test.junit4)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(libs.androidx.junit)
    debugImplementation(libs.androidx.compose.ui.test.manifest)
    debugImplementation(libs.androidx.compose.ui.tooling)
}