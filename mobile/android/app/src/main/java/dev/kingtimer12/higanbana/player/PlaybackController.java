package dev.kingtimer12.higanbana.player;

import android.util.Log;
import android.view.Surface;

import dev.jdtech.mpv.MPVLib;

import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

/**
 * Leva os pedidos da interface ao mpv por uma thread de controle, na ordem em que
 * chegaram: nenhuma chamada ao mpv roda na thread da interface, porque ele pode estar
 * preso num decodificador ou na rede.
 *
 * Os campos sem {@code volatile} só são lidos e escritos na thread de controle.
 */
final class PlaybackController {
    private static final String TAG = "higanbana.mpv";
    /** Quanto a interface espera o mpv soltar a superfície antes de o sistema destruí-la. */
    private static final long DETACH_WAIT_MS = 500;
    /** De quanto em quanto tempo o estado da reprodução vai para o logcat. */
    private static final long STATS_EVERY_S = 10;

    private final MpvEngine mpv;
    private final PlaybackState state;
    private final ScheduledExecutorService control =
            Executors.newSingleThreadScheduledExecutor(r -> new Thread(r, "mpv-control"));

    private String src = "";
    private double startAt;
    private boolean paused;
    /** O arquivo que o mpv tem aberto (ou está abrindo). */
    private String opened = "";
    private boolean surfaceReady;

    PlaybackController(MpvEngine mpv, PlaybackState state) {
        this.mpv = mpv;
        this.state = state;
        // Para de repetir sozinho no shutdown() de close().
        control.scheduleWithFixedDelay(this::logStats, STATS_EVERY_S, STATS_EVERY_S, TimeUnit.SECONDS);
    }

    // --- Pedidos da interface ---

    void setSource(String next) {
        control.execute(() -> src = next);
    }

    void setStartAt(double next) {
        control.execute(() -> startAt = next);
    }

    void setPaused(boolean next) {
        control.execute(() -> {
            paused = next;
            mpv.setPaused(next);
        });
    }

    void selectSubtitle(String next) {
        control.execute(() -> {
            state.sub = next;
            if (state.loaded) mpv.selectSubtitle(next);
        });
    }

    void selectAudio(String next) {
        control.execute(() -> {
            state.audio = next;
            if (state.loaded) mpv.selectAudio(next);
        });
    }

    void seekTo(double seconds) {
        control.execute(() -> mpv.seekTo(seconds));
    }

    /** Abre {@code src} se mudou e já há superfície. */
    void requestLoad() {
        control.execute(this::load);
    }

    // --- Superfície ---

    void surfaceCreated(Surface surface) {
        control.execute(() -> {
            if (state.destroyed) return;
            mpv.attach(surface);
            surfaceReady = true;
            load();
        });
    }

    void surfaceResized(int width, int height) {
        control.execute(() -> mpv.resize(width, height));
    }

    /**
     * O sistema destrói a superfície quando isto retorna: espera o mpv soltá-la, mas
     * pouco; se o núcleo dele estiver preso, a tela segue e o mpv lida com a perda.
     */
    void surfaceDestroyed() {
        try {
            Future<?> released = control.submit(this::releaseSurface);
            released.get(DETACH_WAIT_MS, TimeUnit.MILLISECONDS);
        } catch (Exception e) {
            Log.w(TAG, "o mpv não soltou a superfície a tempo");
        }
    }

    /**
     * Encerra o mpv depois do que já estava na fila (encerrar espera as threads dele)
     * e para a thread de controle.
     */
    void close(MPVLib.EventObserver observer) {
        control.execute(() -> {
            mpv.unobserve(observer);
            mpv.setPaused(true);
            releaseSurface();
            mpv.destroy();
        });
        control.shutdown();
    }

    // --- Thread de controle ---

    private void load() {
        if (state.destroyed || !surfaceReady || src.isEmpty() || src.equals(opened)) return;
        opened = src;
        state.loaded = false;
        mpv.open(src, startAt, paused);
    }

    private void logStats() {
        if (state.loaded && !state.destroyed) Log.i(TAG, mpv.describePlayback());
    }

    private void releaseSurface() {
        if (!surfaceReady) return;
        surfaceReady = false;
        mpv.detach();
    }
}
