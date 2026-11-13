package higanbana

import org.gradle.api.GradleException

/** Uma ABI do Android com o alvo do Rust e o prefixo do clang do NDK que a compilam. */
enum class AndroidAbi(val id: String, val rustTarget: String, private val clangTarget: String) {
    ARM64("arm64-v8a", "aarch64-linux-android", "aarch64-linux-android"),
    ARMV7("armeabi-v7a", "armv7-linux-androideabi", "armv7a-linux-androideabi"),
    X86_64("x86_64", "x86_64-linux-android", "x86_64-linux-android"),
    X86("x86", "i686-linux-android", "i686-linux-android");

    /** `aarch64_linux_android`: o sufixo das variáveis `CC_<alvo>` que o crate `cc` lê. */
    val envSuffix: String get() = rustTarget.replace('-', '_')

    /** `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER` etc. */
    val cargoLinkerVar: String get() = "CARGO_TARGET_${envSuffix.uppercase()}_LINKER"

    /** O clang do NDK que já mira esta ABI e a API mínima. */
    fun clang(toolchainBin: java.io.File, minSdk: Int): java.io.File =
        toolchainBin.resolve("$clangTarget$minSdk-clang")

    companion object {
        fun of(id: String): AndroidAbi =
            entries.firstOrNull { it.id == id }
                ?: throw GradleException("ABI desconhecida: $id (use ${entries.joinToString { it.id }})")

        /** `"arm64-v8a x86_64"` ou `"arm64-v8a,x86_64"`. */
        fun parseList(spec: String): List<AndroidAbi> =
            spec.split(',', ' ').map(String::trim).filter(String::isNotEmpty).map(::of)
    }
}
