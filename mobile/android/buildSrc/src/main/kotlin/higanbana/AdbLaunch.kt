package higanbana

import org.gradle.api.DefaultTask
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.Optional
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import javax.inject.Inject

/**
 * Abre o app no aparelho que o adb vê. Com [reversePort], liga antes a porta do
 * servidor do `nsh dev` (o APK de desenvolvimento busca as telas em 127.0.0.1).
 */
abstract class AdbLaunch @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Internal abstract val adb: RegularFileProperty
    /** `pacote/.Atividade`. */
    @get:Input abstract val component: Property<String>
    @get:Input @get:Optional abstract val reversePort: Property<Int>
    /** Extras de texto da intent (`hwdec=no` vira `--es hwdec no`). */
    @get:Input abstract val stringExtras: ListProperty<String>

    init {
        outputs.upToDateWhen { false }
    }

    @TaskAction
    fun launch() {
        val adb = adb.get().asFile.path
        if (reversePort.isPresent) {
            val port = "tcp:${reversePort.get()}"
            exec.exec { commandLine(adb, "reverse", port, port) }
        }
        val extras = stringExtras.get().flatMap { pair ->
            val (key, value) = pair.split('=', limit = 2).let { it[0] to it.getOrElse(1) { "" } }
            listOf("--es", key, value)
        }
        exec.exec { commandLine(listOf(adb, "shell", "am", "start", "-n", component.get()) + extras) }
    }
}
