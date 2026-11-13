package dev.kingtimer12.higanbana.player;

import com.longbridge.nsh.NshComponentContext;

import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** Os eventos que o componente manda para a tela ({@code components/VideoSurface.tsx}). */
final class PlayerEmitter {
    private final NshComponentContext context;

    PlayerEmitter(NshComponentContext context) {
        this.context = context;
    }

    void loaded(double duration, List<Object> tracks, List<Object> chapters) {
        Map<String, Object> data = new HashMap<>();
        data.put("duration", duration);
        data.put("tracks", tracks);
        data.put("chapters", chapters);
        context.emit("load", data);
    }

    void progress(double position, double duration) {
        Map<String, Object> data = new HashMap<>();
        data.put("position", position);
        data.put("duration", duration);
        context.emit("progress", data);
    }

    void buffering(boolean buffering) {
        Map<String, Object> data = new HashMap<>();
        data.put("buffering", buffering);
        context.emit("buffer", data);
    }

    void ended() {
        context.emit("end", new HashMap<String, Object>());
    }

    void error(String message) {
        Map<String, Object> data = new HashMap<>();
        data.put("message", message);
        context.emit("error", data);
    }
}
