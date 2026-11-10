package dev.kingtimer12.higanbana;

import android.app.Activity;
import android.os.Build;
import android.os.Bundle;
import android.util.Log;
import android.view.View;
import android.view.WindowInsetsController;

import com.longbridge.nsh.NshView;

import java.io.IOException;
import java.io.InputStream;

/**
 * A atividade do app: uma {@link NshView} em tela cheia com o player libmpv
 * registrado como componente nativo ({@code higanbana.mpv}).
 *
 * O APK de release leva o bundle em {@code assets/nsh}; sem ele (build de
 * desenvolvimento), a tela vem do servidor do {@code nsh dev}.
 */
public final class MainActivity extends Activity {
    private static final String TAG = "higanbana";
    private static final String BUNDLE_DIR = "nsh";
    private static final String DEV_SERVER = "ws://127.0.0.1:8790/ws";

    private NshView view;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        edgeToEdge();
        view = new NshView(this);
        view.setErrorListener(message -> Log.e(TAG, message));
        // Um app inteiro, não um módulo embutido: chamadas ao núcleo podem demorar mais que o padrão.
        view.setLimits(0, 0, 30_000, 15_000, 0);
        // `adb shell am start ... --es hwdec no`: decodifica por software (emuladores cujo
        // decodificador de hardware não responde).
        String hwdec = getIntent().getStringExtra("hwdec");
        view.registerComponent(MpvPlayer.NAME, context -> new MpvPlayer(this, context, hwdec));
        setContentView(view);
        if (hasBundle()) {
            view.loadAssetBundle(BUNDLE_DIR);
        } else {
            String url = getIntent().getStringExtra("url");
            view.connectDevServer(url != null ? url : DEV_SERVER);
        }
    }

    private boolean hasBundle() {
        try (InputStream in = getAssets().open(BUNDLE_DIR + "/bundle.nshc")) {
            return true;
        } catch (IOException e) {
            return false;
        }
    }

    /** Desenha atrás das barras do sistema: o app recebe as margens como área segura. */
    @SuppressWarnings("deprecation")
    private void edgeToEdge() {
        if (Build.VERSION.SDK_INT >= 30) {
            getWindow().setDecorFitsSystemWindows(false);
            // O app é sempre escuro: ícones claros nas barras.
            WindowInsetsController controller = getWindow().getDecorView().getWindowInsetsController();
            if (controller != null) {
                controller.setSystemBarsAppearance(0,
                        WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
                                | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS);
            }
        } else {
            getWindow().getDecorView().setSystemUiVisibility(
                    View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                            | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
        }
        getWindow().setStatusBarColor(0);
        getWindow().setNavigationBarColor(0);
    }

    /** Android 12 e anteriores: o app recebe o botão voltar primeiro. */
    @Override
    @SuppressWarnings("deprecation")
    public void onBackPressed() {
        if (view == null || !view.handleBack()) super.onBackPressed();
    }

    @Override
    protected void onDestroy() {
        view.destroy();
        super.onDestroy();
    }
}
