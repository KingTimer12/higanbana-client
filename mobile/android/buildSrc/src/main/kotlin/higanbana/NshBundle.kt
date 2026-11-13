package higanbana

import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.FileSystemOperations
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.process.ExecOperations
import javax.inject.Inject

/**
 * As telas TSX empacotadas (`nsh build <entrada> --release`), em `<outputDir>/nsh/`:
 * a pasta de assets que a MainActivity abre com `loadAssetBundle("nsh")`.
 */
abstract class NshBundle @Inject constructor(
    private val exec: ExecOperations,
    private val files: FileSystemOperations,
) : DefaultTask() {
    @get:Input abstract val nsh: Property<String>
    @get:Input abstract val searchPath: Property<String>
    /** A entrada TSX, relativa a [appDir] (`App.tsx`, `Fixture.tsx`...). */
    @get:Input abstract val entry: Property<String>
    @get:Internal abstract val appDir: DirectoryProperty
    /** Telas, estilos e assets: o que muda o bundle. */
    @get:InputFiles @get:PathSensitive(PathSensitivity.RELATIVE) abstract val sources: ConfigurableFileCollection
    @get:Internal abstract val scratchDir: DirectoryProperty
    @get:OutputDirectory abstract val outputDir: DirectoryProperty

    @TaskAction
    fun bundle() {
        val scratch = scratchDir.get().asFile
        files.delete { delete(scratch) }
        exec.exec {
            workingDir = appDir.get().asFile
            environment("PATH", searchPath.get())
            commandLine(nsh.get(), "build", entry.get(), "--release", "--out", scratch.path)
        }
        files.sync {
            from(scratch) { include("bundle.nshc", "assets/**", "capabilities.json", "app.json") }
            into(outputDir.dir("nsh"))
        }
    }
}
