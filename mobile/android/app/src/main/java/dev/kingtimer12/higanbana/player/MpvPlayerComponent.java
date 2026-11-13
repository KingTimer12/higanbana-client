package dev.kingtimer12.higanbana.player;

import android.app.Activity;
import android.view.View;

import com.longbridge.nsh.NshComponent;
import com.longbridge.nsh.NshComponentContext;

import java.util.Map;

/**
 * O vídeo do player: libmpv numa {@code SurfaceView}, como o componente nativo
 * {@code higanbana.mpv} ({@code components/VideoSurface.tsx}).
 *
 * Props: {@code src}, {@code paused}, {@code startAt} (segundos), {@code sub} e
 * {@code audio} (id da faixa, {@code "no"} ou {@code "auto"}).
 * Eventos: {@code load}, {@code progress}, {@code buffer}, {@code end}, {@code error}.
 * Comando: {@code seek} com {@code { to }}.
 *
 * Só liga as partes: {@link MpvEngine} (o mpv), {@link PlaybackController} (a thread
 * de controle), {@link PlaybackEvents} (eventos do mpv para a tela),
 * {@link VideoSurface} e {@link FullscreenSession}.
 */
public final class MpvPlayerComponent implements NshComponent {
    public static final String NAME = "higanbana.mpv";

    private final PlaybackState state = new PlaybackState();
    private final PlaybackController controller;
    private final PlaybackEvents events;
    private final VideoSurface surface;
    private final FullscreenSession fullscreen;

    /** @param hwdec o decodificador de vídeo do mpv (ver {@code HwdecPolicy}). */
    public MpvPlayerComponent(Activity activity, NshComponentContext context, String hwdec) {
        MpvEngine mpv = new MpvEngine(activity, hwdec);
        events = new PlaybackEvents(mpv, state, new PlayerEmitter(context));
        mpv.observe(events);
        controller = new PlaybackController(mpv, state);
        surface = new VideoSurface(activity, controller, state);
        fullscreen = new FullscreenSession(activity);
        fullscreen.enter();
    }

    @Override
    public View view() {
        return surface.view();
    }

    @Override
    public void setProp(String name, Object value) {
        if (state.destroyed) return;
        switch (name) {
            case "src":
                controller.setSource(value instanceof String ? (String) value : "");
                // As props chegam uma a uma: abre o arquivo depois de todas, uma vez.
                surface.post(controller::requestLoad);
                break;
            case "startAt":
                controller.setStartAt(value instanceof Number ? ((Number) value).doubleValue() : 0);
                break;
            case "paused":
                controller.setPaused(Boolean.TRUE.equals(value));
                break;
            case "sub":
                controller.selectSubtitle(value instanceof String ? (String) value : "auto");
                break;
            case "audio":
                controller.selectAudio(value instanceof String ? (String) value : "auto");
                break;
            default:
                break;
        }
    }

    @Override
    public void command(String name, Object args) {
        if (state.destroyed || !"seek".equals(name) || !(args instanceof Map)) return;
        Object to = ((Map<?, ?>) args).get("to");
        if (to instanceof Number) controller.seekTo(((Number) to).doubleValue());
    }

    @Override
    public void destroy() {
        if (state.destroyed) return;
        state.destroyed = true;
        surface.release();
        fullscreen.leave();
        controller.close(events);
    }
}
