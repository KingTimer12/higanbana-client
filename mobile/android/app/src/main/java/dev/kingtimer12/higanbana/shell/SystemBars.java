package dev.kingtimer12.higanbana.shell;

import android.os.Build;
import android.view.View;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowInsetsController;

/** As barras de status e de navegação de uma janela. */
public final class SystemBars {
    private final Window window;

    public SystemBars(Window window) {
        this.window = window;
    }

    /** Desenha atrás das barras do sistema: o app recebe as margens como área segura. */
    @SuppressWarnings("deprecation")
    public void drawBehind() {
        if (Build.VERSION.SDK_INT >= 30) {
            window.setDecorFitsSystemWindows(false);
            // O app é sempre escuro: ícones claros nas barras.
            WindowInsetsController controller = controller();
            if (controller != null) {
                controller.setSystemBarsAppearance(0,
                        WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
                                | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS);
            }
        } else {
            window.getDecorView().setSystemUiVisibility(
                    View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                            | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
        }
        window.setStatusBarColor(0);
        window.setNavigationBarColor(0);
    }

    /** Esconde as barras; um deslize da borda as mostra por um instante. */
    public void hide() {
        if (Build.VERSION.SDK_INT < 30) return;
        WindowInsetsController controller = controller();
        if (controller == null) return;
        controller.hide(WindowInsets.Type.systemBars());
        controller.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
    }

    public void show() {
        if (Build.VERSION.SDK_INT < 30) return;
        WindowInsetsController controller = controller();
        if (controller != null) controller.show(WindowInsets.Type.systemBars());
    }

    private WindowInsetsController controller() {
        return window.getDecorView().getWindowInsetsController();
    }
}
