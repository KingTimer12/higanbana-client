package higanbana

import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import javax.inject.Inject

/**
 * `nsh check`: gera os fontes do runtime em `.nsh/rust` (o crate que o cargo compila
 * para o Android) e confere TSX e Rust. Sempre roda: quem sabe se mudou algo é o nsh.
 */
abstract class NshCheck @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Input abstract val nsh: Property<String>
    @get:Input abstract val searchPath: Property<String>
    @get:Internal abstract val appDir: DirectoryProperty

    init {
        outputs.upToDateWhen { false }
    }

    @TaskAction
    fun check() {
        exec.exec {
            workingDir = appDir.get().asFile
            environment("PATH", searchPath.get())
            commandLine(nsh.get(), "check")
        }
    }
}
