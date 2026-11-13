package dev.kingtimer12.higanbana.player;

import android.app.Activity;
import android.content.pm.ActivityInfo;
import android.view.WindowManager;

import dev.kingtimer12.higanbana.shell.SystemBars;

/** Enquanto o player existe: tela deitada, sem barras do sistema e acesa. */
final class FullscreenSession {
    private final Activity activity;
    private final SystemBars bars;
    private final int previousOrientation;

    FullscreenSession(Activity activity) {
        this.activity = activity;
        bars = new SystemBars(activity.getWindow());
        previousOrientation = activity.getRequestedOrientation();
    }

    void enter() {
        activity.setRequestedOrientation(ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE);
        activity.getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        bars.hide();
    }

    void leave() {
        activity.setRequestedOrientation(previousOrientation);
        activity.getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        bars.show();
    }
}
