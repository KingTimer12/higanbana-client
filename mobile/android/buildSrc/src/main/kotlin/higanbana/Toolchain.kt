package higanbana

import org.gradle.api.GradleException
import java.io.File

/**
 * Onde estão o `nsh` e o `cargo`. O daemon do Gradle aberto pelo Android Studio não
 * herda o PATH do shell, então além do PATH procura nas pastas de instalação comuns.
 */
class Toolchain(val nsh: File, val cargo: File) {
    /** A instalação do NSH (`~/.nsh`): o binário fica em `bin/` e os runtimes em `embed/`. */
    val nshHome: File get() = nsh.canonicalFile.parentFile.parentFile

    /** O runtime Android do NSH: `nsh.aar` ou, para o servidor de desenvolvimento, `nsh-dev.aar`. */
    fun nshAar(dev: Boolean): File {
        val aar = nshHome.resolve("embed/android/${if (dev) "nsh-dev" else "nsh"}.aar")
        if (!aar.isFile) throw GradleException("runtime Android do NSH não encontrado em $aar")
        return aar
    }

    /** PATH para os processos filhos: as pastas do nsh e do cargo na frente do PATH atual. */
    fun path(): String =
        (listOf(nsh.parentFile, cargo.parentFile).map(File::getPath) + listOfNotNull(System.getenv("PATH")))
            .distinct()
            .joinToString(File.pathSeparator)

    companion object {
        /** `nshPath`/`cargoPath` (propriedades do Gradle) têm prioridade sobre a busca. */
        fun locate(nshPath: String?, cargoPath: String?): Toolchain {
            val home = File(System.getProperty("user.home"))
            return Toolchain(
                nsh = nshPath?.let(::File) ?: find("nsh", home.resolve(".nsh/bin"), home.resolve(".local/bin")),
                cargo = cargoPath?.let(::File) ?: find("cargo", home.resolve(".cargo/bin")),
            )
        }

        private fun find(name: String, vararg extra: File): File {
            val dirs = System.getenv("PATH").orEmpty().split(File.pathSeparator).filter(String::isNotEmpty).map(::File) +
                extra + listOf(File("/opt/homebrew/bin"), File("/usr/local/bin"))
            return dirs.map { it.resolve(name) }.firstOrNull { it.canExecute() }
                ?: throw GradleException("`$name` não encontrado no PATH nem em ${extra.joinToString()}")
        }
    }
}
