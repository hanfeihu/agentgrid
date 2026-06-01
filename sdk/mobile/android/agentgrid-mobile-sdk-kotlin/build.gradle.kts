plugins {
    id("com.android.library")
    kotlin("android")
}

android {
    namespace = "io.agentgrid.mobile"
    compileSdk = 35

    defaultConfig {
        minSdk = 23
    }
}

dependencies {
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
}

