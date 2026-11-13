import groovy.json.JsonSlurper
import higanbana.AdbLaunch
import higanbana.AndroidAbi
import higanbana.CargoNdkBuild
import higanbana.NshBundle
import higanbana.NshCheck
import higanbana.Toolchain

// O APK do higanbana: o runtime do NSH com o crate Rust do app (libnsh.so, via cargo + NDK),
// as telas TSX (nsh build), o player libmpv (Maven) e o código nativo em C++ (CMake).
//
//   ./gradlew :app:assembleBundledRelease          APK com as telas empacotadas
//   ./gradlew :app:runBundledRelease               instala e abre no aparelho
//   ./gradlew :app:runDevServerRelease             telas do `nsh dev --server-only` (hot reload)
//   ./gradlew :app:runBundledRelease -Phiganbana.entry=Fixture.tsx    acervo de mentira
//   ./gradlew ... -Phiganbana.abis=arm64-v8a,x86_64 -Phiganbana.hwdec=no

plugins {
    alias(libs.plugins.android.application)
}

/** `mobile/`: o app NSH (nsh.json, telas, crate Rust). */
val mobileDir: File = rootDir.parentFile
val minSdkVersion = 28
val targetAbis = AndroidAbi.parseList(providers.gradleProperty("higanbana.abis").getOrElse("arm64-v8a"))
val toolchain = Toolchain.locate(
    providers.gradleProperty("higanbana.nsh").orNull,
    providers.gradleProperty("higanbana.cargo").orNull,
)

/** Versão do app: a mesma do nsh.json (`version` e `build`). */
@Suppress("UNCHECKED_CAST")
val nshManifest = JsonSlurper().parse(mobileDir.resolve("nsh.json")) as Map<String, Any?>

android {
    namespace = "dev.kingtimer12.higanbana"
    ndkVersion = "28.2.13676358"
    compileSdk {
        version = release(36) {
            minorApiLevel = 1
        }
    }

    defaultConfig {
        applicationId = "dev.kingtimer12.higanbana"
        minSdk = minSdkVersion
        targetSdk = 36
        versionCode = (nshManifest["build"] as? Number)?.toInt() ?: 1
        versionName = nshManifest["version"] as? String ?: "0.1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk {
            abiFilters += targetAbis.map { it.id }
        }
        externalNativeBuild {
            cmake {
                cppFlags += "-std=c++17"
            }
        }
    }

    // De onde vêm as telas: do próprio APK ou do servidor do `nsh dev`.
    flavorDimensions += "screens"
    productFlavors {
        create("bundled") {
            dimension = "screens"
            buildConfigField("boolean", "DEV_SERVER", "false")
        }
        create("devServer") {
            dimension = "screens"
            versionNameSuffix = "-dev"
            // Só este flavor busca telas fora do APK (e só em loopback: ver DevServerScreens).
            buildConfigField("boolean", "DEV_SERVER", "true")
        }
    }

    signingConfigs {
        val keystore = providers.gradleProperty("higanbana.keystore").orElse(providers.environmentVariable("KEYSTORE")).orNull
        if (keystore != null) {
            create("upload") {
                val password = providers.environmentVariable("KEYSTORE_PASSWORD").getOrElse("android")
                storeFile = file(keystore)
                storePassword = password
                keyAlias = providers.gradleProperty("higanbana.keyAlias").getOrElse("androiddebugkey")
                keyPassword = password
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            // Sem chave própria, assina com a de debug: o APK de release instala direto no aparelho.
            signingConfig = signingConfigs.findByName("upload") ?: signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            version = "3.22.1"
        }
    }
    buildFeatures {
        buildConfig = true
    }
    packaging {
        jniLibs {
            // Bibliotecas extraídas na instalação (o antigo extractNativeLibs="true").
            useLegacyPackaging = true
        }
    }
}

// --- NSH: as classes Java do runtime (o libnsh.so do AAR não entra: o nosso traz o crate do app) ---

val nshClasses = mapOf("bundled" to false, "devServer" to true).mapValues { (flavor, dev) ->
    val extract = tasks.register<Sync>("extractNshClasses${flavor.replaceFirstChar(Char::uppercase)}") {
        from(zipTree(toolchain.nshAar(dev))) { include("classes.jar") }
        into(layout.buildDirectory.dir("nsh/$flavor"))
    }
    files(layout.buildDirectory.file("nsh/$flavor/classes.jar")).builtBy(extract)
}

dependencies {
    "bundledImplementation"(nshClasses.getValue("bundled"))
    "devServerImplementation"(nshClasses.getValue("devServer"))
    implementation(libs.libmpv)
    testImplementation(libs.junit)
    androidTestImplementation(libs.ext.junit)
    androidTestImplementation(libs.espresso.core)
}

// --- Telas e Rust ---

val nshCheck = tasks.register<NshCheck>("nshCheck") {
    group = "higanbana"
    description = "Gera os fontes do runtime em .nsh/rust e confere TSX e Rust (nsh check)."
    nsh.set(toolchain.nsh.path)
    searchPath.set(toolchain.path())
    appDir.set(mobileDir)
}

val nshBundle = tasks.register<NshBundle>("nshBundle") {
    group = "higanbana"
    description = "Empacota as telas TSX (nsh build --release)."
    nsh.set(toolchain.nsh.path)
    searchPath.set(toolchain.path())
    entry.set(providers.gradleProperty("higanbana.entry").orElse("App.tsx"))
    appDir.set(mobileDir)
    sources.from(fileTree(mobileDir) {
        include("*.tsx", "*.ts", "*.json", "pages/**", "components/**", "lib/**", "fixture/**", "assets/**")
        exclude(".nsh/**", "target/**", "android/**")
    })
    scratchDir.set(layout.buildDirectory.dir("nsh/scratch"))
    outputDir.set(layout.buildDirectory.dir("generated/nsh/assets"))
}

/** Um libnsh.so por flavor: o de desenvolvimento liga o cliente do servidor do `nsh dev`. */
val cargoBuilds = mapOf("bundled" to false, "devServer" to true).mapValues { (flavor, dev) ->
    tasks.register<CargoNdkBuild>("cargoBuild${flavor.replaceFirstChar(Char::uppercase)}") {
        group = "higanbana"
        description = "Compila libnsh.so (runtime do NSH + crate do app) para ${targetAbis.joinToString { it.id }}."
        dependsOn(nshCheck)
        cargo.set(toolchain.cargo.path)
        searchPath.set(toolchain.path())
        manifest.set(File(mobileDir, ".nsh/rust/Cargo.toml"))
        targetDir.set(File(mobileDir, "target"))
        cratePackage.set("nsh-app-higanbana-mobile-android")
        crateLib.set("nsh_app_higanbana_mobile_android")
        outputLib.set("libnsh.so")
        abis.set(targetAbis.map { it.id })
        features.set(if (dev) listOf("dev") else emptyList())
        minSdk.set(minSdkVersion)
        ndkDirectory.set(androidComponents.sdkComponents.ndkDirectory)
        outputDir.set(layout.buildDirectory.dir("generated/rust/$flavor/jniLibs"))
    }
}

androidComponents {
    onVariants { variant ->
        val flavor = variant.flavorName ?: "bundled"
        val dev = flavor == "devServer"
        val name = variant.name.replaceFirstChar(Char::uppercase)

        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargoBuilds.getValue(flavor), CargoNdkBuild::outputDir)
        if (!dev) variant.sources.assets?.addGeneratedSourceDirectory(nshBundle, NshBundle::outputDir)

        tasks.register<AdbLaunch>("run$name") {
            group = "higanbana"
            description = "Instala e abre ${variant.name} no aparelho."
            dependsOn("install$name")
            adb.set(androidComponents.sdkComponents.adb)
            component.set(variant.applicationId.map { "$it/dev.kingtimer12.higanbana.MainActivity" })
            if (dev) reversePort.set(providers.gradleProperty("higanbana.devPort").map(String::toInt).orElse(8790))
            stringExtras.set(providers.gradleProperty("higanbana.hwdec").map { listOf("hwdec=$it") }.orElse(emptyList()))
        }
    }
}
