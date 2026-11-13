package higanbana

import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import java.io.File
import javax.inject.Inject

/**
 * `libnsh.so` por ABI: o runtime do NSH ligado ao crate do app (`.nsh/rust`),
 * compilado com o clang do NDK e despido de símbolos, em `<outputDir>/<abi>/`.
 *
 * Sempre roda: o cargo sabe melhor que o Gradle o que mudou no Rust.
 */
abstract class CargoNdkBuild @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Input abstract val cargo: Property<String>
    @get:Input abstract val searchPath: Property<String>
    /** O `Cargo.toml` do workspace gerado pelo nsh (`.nsh/rust/Cargo.toml`). */
    @get:Internal abstract val manifest: RegularFileProperty
    @get:Internal abstract val targetDir: DirectoryProperty
    @get:Input abstract val cratePackage: Property<String>
    /** Nome do `.so` que o cargo gera (`lib<crateLib>.so`). */
    @get:Input abstract val crateLib: Property<String>
    /** Nome com que o `.so` entra no APK (o runtime do NSH carrega `libnsh.so`). */
    @get:Input abstract val outputLib: Property<String>
    @get:Input abstract val abis: ListProperty<String>
    @get:Input abstract val features: ListProperty<String>
    @get:Input abstract val minSdk: Property<Int>
    @get:Internal abstract val ndkDirectory: DirectoryProperty
    @get:OutputDirectory abstract val outputDir: DirectoryProperty

    init {
        outputs.upToDateWhen { false }
    }

    @TaskAction
    fun build() {
        val manifest = manifest.get().asFile
        if (!manifest.isFile) throw GradleException("fontes do runtime ausentes em ${manifest.parentFile} (rode `nsh check`)")
        val toolchain = toolchainBin(ndkDirectory.get().asFile)
        val sysroot = toolchain.parentFile.resolve("sysroot")

        for (abi in abis.get().map(AndroidAbi::of)) {
            val clang = abi.clang(toolchain, minSdk.get())
            if (!clang.canExecute()) throw GradleException("compilador do NDK ausente: $clang")
            logger.lifecycle("→ compilando ${outputLib.get()} para ${abi.id}")
            exec.exec {
                environment("PATH", searchPath.get())
                environment("CC_${abi.envSuffix}", clang.path)
                environment("CXX_${abi.envSuffix}", "${clang.path}++")
                environment("AR_${abi.envSuffix}", toolchain.resolve("llvm-ar").path)
                environment(abi.cargoLinkerVar, clang.path)
                // O bindgen (QuickJS) precisa dos cabeçalhos do NDK, não dos do sistema.
                environment("CLANG_PATH", clang.path)
                environment("BINDGEN_EXTRA_CLANG_ARGS_${abi.envSuffix}", "--sysroot=${sysroot.path}")
                val args = mutableListOf(
                    cargo.get(), "build",
                    "--manifest-path", manifest.path,
                    "--target-dir", targetDir.get().asFile.path,
                    "-p", cratePackage.get(),
                    "--target", abi.rustTarget,
                    "--release",
                )
                if (features.get().isNotEmpty()) args += listOf("--features", features.get().joinToString(","))
                commandLine(args)
            }
            val built = targetDir.get().asFile.resolve("${abi.rustTarget}/release/lib${crateLib.get()}.so")
            val out = outputDir.get().asFile.resolve("${abi.id}/${outputLib.get()}")
            out.parentFile.mkdirs()
            exec.exec {
                commandLine(toolchain.resolve("llvm-strip").path, "--strip-unneeded", "-o", out.path, built.path)
            }
        }
    }

    /** `ndk/<versão>/toolchains/llvm/prebuilt/<host>/bin`. */
    private fun toolchainBin(ndk: File): File =
        ndk.resolve("toolchains/llvm/prebuilt").listFiles()?.filter(File::isDirectory)?.maxByOrNull(File::getName)
            ?.resolve("bin")
            ?: throw GradleException("toolchain LLVM não encontrada no NDK em $ndk")
}
