package dev.kingtimer12.higanbana;

import android.app.Activity;
import android.content.pm.ActivityInfo;
import android.os.Build;
import android.os.SystemClock;
import android.util.Log;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;

import com.longbridge.nsh.NshComponent;
import com.longbridge.nsh.NshComponentContext;

import dev.jdtech.mpv.MPVLib;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;

/**
 * O vídeo do player: libmpv numa {@link SurfaceView}, como o componente nativo
 * {@code higanbana.mpv} ({@code components/VideoSurface.tsx}).
 *
 * Props: {@code src}, {@code paused}, {@code startAt} (segundos), {@code sub} e
 * {@code audio} (id da faixa, {@code "no"} ou {@code "auto"}).
 * Eventos: {@code load}, {@code progress}, {@code buffer}, {@code end}, {@code error}.
 * Comando: {@code seek} com {@code { to }}.
 *
 * Enquanto existe, a tela fica deitada, sem barras do sistema e acesa.
 *
 * Nenhuma chamada ao mpv roda na thread da interface: a API dele é síncrona e
 * espera o núcleo do player, que pode estar preso num decodificador ou na rede.
 * Tudo passa por uma thread de controle, na ordem em que foi pedido.
 */
final class MpvPlayer implements NshComponent, SurfaceHolder.Callback, MPVLib.EventObserver {
    static final String NAME = "higanbana.mpv";
    private static final String TAG = "higanbana.mpv";
    private static final long PROGRESS_EVERY_MS = 250;
    /** Quanto a interface espera o mpv soltar a superfície antes de o sistema destruí-la. */
    private static final long DETACH_WAIT_MS = 500;

    private final Activity activity;
    private final NshComponentContext context;
    private final SurfaceView surface;
    private final MPVLib mpv;
    private final int previousOrientation;
    /** Onde rodam as chamadas ao mpv pedidas pela interface, em ordem. */
    private final ExecutorService control = Executors.newSingleThreadExecutor(r -> new Thread(r, "mpv-control"));

    // Só a thread de controle lê e escreve.
    private String src = "";
    private double startAt;
    private boolean paused;
    /** O arquivo que o mpv tem aberto (ou está abrindo). */
    private String opened = "";
    private boolean surfaceReady;

    private volatile String sub = "auto";
    private volatile String audio = "auto";
    private volatile boolean destroyed;

    // Escritos na thread de eventos do mpv.
    private volatile boolean loaded;
    private volatile double duration;
    private volatile long progressAt;

    /**
     * @param hwdec o decodificador de vídeo do mpv; nulo usa o do aparelho
     *     ({@code mediacodec-copy}). {@code "no"} decodifica por software.
     */
    MpvPlayer(Activity activity, NshComponentContext context, String hwdec) {
        this.activity = activity;
        this.context = context;
        previousOrientation = activity.getRequestedOrientation();

        mpv = MPVLib.create(activity);
        // Só o player: sem scripts, sem configuração do usuário, sem controles próprios.
        mpv.setOptionString("config", "no");
        mpv.setOptionString("load-scripts", "no");
        mpv.setOptionString("osc", "no");
        mpv.setOptionString("ytdl", "no");
        mpv.setOptionString("input-default-bindings", "no");
        mpv.setOptionString("msg-level", "all=warn");
        // Vídeo só depois que houver superfície.
        mpv.setOptionString("vo", "null");
        mpv.setOptionString("force-window", "no");
        mpv.setOptionString("gpu-context", "android");
        mpv.setOptionString("opengl-es", "yes");
        // `-copy`: os quadros voltam para a memória e o vo=gpu os desenha, com as legendas
        // por cima; o mediacodec direto precisa de uma superfície só dele.
        mpv.setOptionString("hwdec", hwdec != null ? hwdec : "mediacodec-copy");
        mpv.setOptionString("ao", "audiotrack,opensles");
        mpv.setOptionString("idle", "yes");
        // Parado no último quadro ao fim: o fim chega por `eof-reached`, e a tela decide o que vem depois.
        mpv.setOptionString("keep-open", "yes");
        mpv.setOptionString("cache", "yes");
        mpv.setOptionString("demuxer-max-bytes", "64MiB");
        mpv.setOptionString("demuxer-max-back-bytes", "16MiB");
        mpv.init();
        mpv.addObserver(this);
        mpv.observeProperty("time-pos", MPVLib.MpvFormat.MPV_FORMAT_DOUBLE);
        mpv.observeProperty("duration", MPVLib.MpvFormat.MPV_FORMAT_DOUBLE);
        mpv.observeProperty("paused-for-cache", MPVLib.MpvFormat.MPV_FORMAT_FLAG);
        mpv.observeProperty("eof-reached", MPVLib.MpvFormat.MPV_FORMAT_FLAG);

        surface = new SurfaceView(activity);
        surface.getHolder().addCallback(this);
        enterFullscreen();
    }

    @Override
    public View view() {
        return surface;
    }

    // --- Props e comandos (chegam na thread da interface, rodam na de controle) ---

    @Override
    public void setProp(String name, Object value) {
        if (destroyed) return;
        switch (name) {
            case "src": {
                String next = value instanceof String ? (String) value : "";
                control.execute(() -> src = next);
                // As props chegam uma a uma: abre o arquivo depois de todas, uma vez.
                surface.post(() -> control.execute(this::load));
                break;
            }
            case "startAt": {
                double next = value instanceof Number ? ((Number) value).doubleValue() : 0;
                control.execute(() -> startAt = next);
                break;
            }
            case "paused": {
                boolean next = Boolean.TRUE.equals(value);
                control.execute(() -> {
                    paused = next;
                    mpv.setPropertyBoolean("pause", next);
                });
                break;
            }
            case "sub": {
                String next = value instanceof String ? (String) value : "auto";
                control.execute(() -> {
                    sub = next;
                    if (loaded) mpv.setPropertyString("sid", next);
                });
                break;
            }
            case "audio": {
                String next = value instanceof String ? (String) value : "auto";
                control.execute(() -> {
                    audio = next;
                    if (loaded) mpv.setPropertyString("aid", next);
                });
                break;
            }
            default:
                break;
        }
    }

    @Override
    public void command(String name, Object args) {
        if (destroyed || !"seek".equals(name) || !(args instanceof Map)) return;
        Object to = ((Map<?, ?>) args).get("to");
        if (!(to instanceof Number)) return;
        String seconds = String.valueOf(((Number) to).doubleValue());
        control.execute(() -> mpv.command(new String[] {"seek", seconds, "absolute"}));
    }

    /** Na thread de controle. */
    private void load() {
        if (destroyed || !surfaceReady || src.isEmpty() || src.equals(opened)) return;
        opened = src;
        loaded = false;
        mpv.setPropertyString("start", startAt > 0 ? String.valueOf(startAt) : "none");
        mpv.setPropertyString("sid", "auto");
        mpv.setPropertyString("aid", "auto");
        mpv.setPropertyBoolean("pause", paused);
        mpv.command(new String[] {"loadfile", src, "replace"});
    }

    /** Na thread de controle. */
    private void releaseSurface() {
        if (!surfaceReady) return;
        surfaceReady = false;
        mpv.setPropertyString("vo", "null");
        mpv.setOptionString("force-window", "no");
        mpv.detachSurface();
    }

    // --- Superfície (thread da interface) ---

    @Override
    public void surfaceCreated(SurfaceHolder holder) {
        if (destroyed) return;
        android.view.Surface target = holder.getSurface();
        control.execute(() -> {
            if (destroyed) return;
            mpv.attachSurface(target);
            mpv.setOptionString("force-window", "yes");
            mpv.setPropertyString("vo", "gpu");
            surfaceReady = true;
            load();
        });
    }

    @Override
    public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        if (destroyed) return;
        control.execute(() -> mpv.setPropertyString("android-surface-size", width + "x" + height));
    }

    @Override
    public void surfaceDestroyed(SurfaceHolder holder) {
        // O sistema destrói a superfície quando isto retorna: espera o mpv soltá-la, mas
        // pouco; se o núcleo dele estiver preso, a tela segue e o mpv lida com a perda.
        if (destroyed) return;
        try {
            Future<?> released = control.submit(this::releaseSurface);
            released.get(DETACH_WAIT_MS, TimeUnit.MILLISECONDS);
        } catch (Exception e) {
            Log.w(TAG, "o mpv não soltou a superfície a tempo");
        }
    }

    // --- Eventos do mpv (thread de eventos do mpv) ---

    @Override
    public void eventProperty(String property, double value) {
        if (destroyed) return;
        if ("duration".equals(property)) {
            duration = value;
        } else if ("time-pos".equals(property)) {
            long now = SystemClock.elapsedRealtime();
            if (now - progressAt < PROGRESS_EVERY_MS) return;
            progressAt = now;
            Map<String, Object> data = new HashMap<>();
            data.put("position", value);
            data.put("duration", duration);
            context.emit("progress", data);
        }
    }

    @Override
    public void eventProperty(String property, boolean value) {
        if (destroyed) return;
        if ("paused-for-cache".equals(property)) {
            Map<String, Object> data = new HashMap<>();
            data.put("buffering", value);
            context.emit("buffer", data);
        } else if ("eof-reached".equals(property) && value && loaded) {
            context.emit("end", new HashMap<String, Object>());
        }
    }

    @Override
    public void eventProperty(String property) {}

    @Override
    public void eventProperty(String property, long value) {}

    @Override
    public void eventProperty(String property, String value) {}

    @Override
    public void event(int id) {
        if (destroyed) return;
        if (id == MPVLib.MpvEvent.MPV_EVENT_START_FILE) {
            loaded = false;
        } else if (id == MPVLib.MpvEvent.MPV_EVENT_FILE_LOADED) {
            loaded = true;
            // A retomada vale para este arquivo só.
            mpv.setPropertyString("start", "none");
            if (!"auto".equals(sub)) mpv.setPropertyString("sid", sub);
            if (!"auto".equals(audio)) mpv.setPropertyString("aid", audio);
            Double length = mpv.getPropertyDouble("duration");
            if (length != null) duration = length;
            Map<String, Object> data = new HashMap<>();
            data.put("duration", duration);
            data.put("tracks", tracks());
            data.put("chapters", chapters());
            context.emit("load", data);
        } else if (id == MPVLib.MpvEvent.MPV_EVENT_END_FILE && !loaded) {
            // Terminou sem ter aberto: o arquivo não existe, a rede caiu ou o formato não abre.
            Map<String, Object> data = new HashMap<>();
            data.put("message", "O arquivo não abriu. Confira a conexão e tente de novo.");
            context.emit("error", data);
        }
    }

    /** `track-list` do mpv: `[{ id, type, lang, title, selected }]`. */
    private List<Object> tracks() {
        List<Object> out = new ArrayList<>();
        try {
            JSONArray list = new JSONArray(orEmpty(mpv.getPropertyString("track-list")));
            for (int i = 0; i < list.length(); i++) {
                JSONObject t = list.getJSONObject(i);
                Map<String, Object> track = new HashMap<>();
                track.put("id", t.optLong("id"));
                track.put("type", t.optString("type"));
                track.put("lang", t.has("lang") ? t.optString("lang") : null);
                track.put("title", t.has("title") ? t.optString("title") : null);
                track.put("selected", t.optBoolean("selected"));
                out.add(track);
            }
        } catch (JSONException e) {
            Log.w(TAG, "track-list ilegível", e);
        }
        return out;
    }

    /** `chapter-list` do mpv: `[{ title, time }]`. */
    private List<Object> chapters() {
        List<Object> out = new ArrayList<>();
        try {
            JSONArray list = new JSONArray(orEmpty(mpv.getPropertyString("chapter-list")));
            for (int i = 0; i < list.length(); i++) {
                JSONObject c = list.getJSONObject(i);
                Map<String, Object> chapter = new HashMap<>();
                chapter.put("title", c.optString("title"));
                chapter.put("time", c.optDouble("time", 0));
                out.add(chapter);
            }
        } catch (JSONException e) {
            Log.w(TAG, "chapter-list ilegível", e);
        }
        return out;
    }

    private static String orEmpty(String json) {
        return json == null || json.isEmpty() ? "[]" : json;
    }

    // --- Tela ---

    private void enterFullscreen() {
        activity.setRequestedOrientation(ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE);
        activity.getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController controller = activity.getWindow().getDecorView().getWindowInsetsController();
            if (controller != null) {
                controller.hide(WindowInsets.Type.systemBars());
                controller.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        }
    }

    private void leaveFullscreen() {
        activity.setRequestedOrientation(previousOrientation);
        activity.getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController controller = activity.getWindow().getDecorView().getWindowInsetsController();
            if (controller != null) controller.show(WindowInsets.Type.systemBars());
        }
    }

    @Override
    public void destroy() {
        if (destroyed) return;
        destroyed = true;
        surface.getHolder().removeCallback(this);
        leaveFullscreen();
        // Encerrar espera as threads do mpv: na de controle, depois do que já estava na fila.
        control.execute(() -> {
            mpv.removeObserver(this);
            mpv.setPropertyBoolean("pause", true);
            releaseSurface();
            mpv.destroy();
        });
        control.shutdown();
    }
}
