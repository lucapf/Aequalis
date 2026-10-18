plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "io.lucapf.aequalis"
    compileSdk {
        version = release(37)
    }

    defaultConfig {
        applicationId = "io.lucapf.aequalis"
        minSdk = 26
        targetSdk = 37
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        // The fast build, for using the app (scripts/install-android.sh): compiled ahead of time,
        // where the debug build runs in the interpreter so that a debugger can stop it anywhere.
        release {
            // Signed with the debug key, as the debug build: either installs over the other, the
            // app's data kept. The Play Store will need an upload key of its own.
            signingConfig = signingConfigs.getByName("debug")
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