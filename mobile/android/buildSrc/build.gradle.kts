// Tarefas do build do app (NSH, cargo + NDK, adb) que o app/build.gradle.kts liga às variantes.
plugins {
    `kotlin-dsl`
}

repositories {
    mavenCentral()
    gradlePluginPortal()
}
