package dev.kingtimer12.higanbana.shell;

import android.app.Activity;
import android.util.Log;

import com.longbridge.nsh.NshComponent;
import com.longbridge.nsh.NshView;

/** Dona da {@link NshView}: configura o runtime, registra componentes e mostra as telas. */
public final class NshHost {
    private static final String TAG = "higanbana";

    private final Activity activity;
    private final NshView view;

    public NshHost(Activity activity) {
        this.activity = activity;
        view = new NshView(activity);
        view.setErrorListener(message -> Log.e(TAG, message));
        // Vira o `process.dataDir` das telas: sem ele o núcleo procura a pasta de
        // dados do sistema, que o Android não tem (sessão, cache e progresso ficam aqui).
        view.setStorageDirectory(activity.getFilesDir());
        // Um app inteiro, não um módulo embutido: chamadas ao núcleo podem demorar mais que o padrão.
        view.setLimits(0, 0, 30_000, 15_000, 0);
    }

    public void register(String name, NshComponent.Factory factory) {
        view.registerComponent(name, factory);
    }

    /** Põe a view na atividade e carrega as telas de {@code source}. */
    public void show(ScreenSource source) {
        activity.setContentView(view);
        source.load(view);
    }

    public boolean handleBack() {
        return view.handleBack();
    }

    public void destroy() {
        view.destroy();
    }
}
