package dev.kingtimer12.higanbana.player;

import android.content.Context;
import android.view.Surface;

import dev.jdtech.mpv.MPVLib;

/**
 * O libmpv configurado como só um player (sem scripts, configuração do usuário nem
 * controles próprios), com as operações que o app usa.
 *
 * A API do mpv é síncrona e espera o núcleo do player: quem chama decide a thread
 * (ver {@link PlaybackController}).
 */
final class MpvEngine {
    private final MPVLib mpv;

    /** @param hwdec o decodificador de vídeo do mpv ({@code mediacodec-copy}, {@code no}...). */
    MpvEngine(Context context, String hwdec) {
        mpv = MPVLib.create(context);
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
        mpv.setOptionString("hwdec", hwdec);
        mpv.setOptionString("ao", "audiotrack,opensles");
        mpv.setOptionString("idle", "yes");
        // Parado no último quadro ao fim: o fim chega por `eof-reached`, e a tela decide o que vem depois.
        mpv.setOptionString("keep-open", "yes");
        mpv.setOptionString("cache", "yes");
        mpv.setOptionString("demuxer-max-bytes", "64MiB");
        mpv.setOptionString("demuxer-max-back-bytes", "16MiB");
        mpv.init();
    }

    /** Passa a receber os eventos e as propriedades que o player acompanha. */
    void observe(MPVLib.EventObserver observer) {
        mpv.addObserver(observer);
        mpv.observeProperty("time-pos", MPVLib.MpvFormat.MPV_FORMAT_DOUBLE);
        mpv.observeProperty("duration", MPVLib.MpvFormat.MPV_FORMAT_DOUBLE);
        mpv.observeProperty("paused-for-cache", MPVLib.MpvFormat.MPV_FORMAT_FLAG);
        mpv.observeProperty("eof-reached", MPVLib.MpvFormat.MPV_FORMAT_FLAG);
    }

    void unobserve(MPVLib.EventObserver observer) {
        mpv.removeObserver(observer);
    }

    // --- Reprodução ---

    /** Abre {@code url} no lugar do atual, a partir de {@code startAt} segundos. */
    void open(String url, double startAt, boolean paused) {
        mpv.setPropertyString("start", startAt > 0 ? String.valueOf(startAt) : "none");
        mpv.setPropertyString("sid", "auto");
        mpv.setPropertyString("aid", "auto");
        mpv.setPropertyBoolean("pause", paused);
        mpv.command(new String[] {"loadfile", url, "replace"});
    }

    /** A retomada vale só para o arquivo que acabou de abrir. */
    void forgetStart() {
        mpv.setPropertyString("start", "none");
    }

    void setPaused(boolean paused) {
        mpv.setPropertyBoolean("pause", paused);
    }

    void seekTo(double seconds) {
        mpv.command(new String[] {"seek", String.valueOf(seconds), "absolute"});
    }

    void selectSubtitle(String id) {
        mpv.setPropertyString("sid", id);
    }

    void selectAudio(String id) {
        mpv.setPropertyString("aid", id);
    }

    // --- Propriedades do arquivo aberto ---

    Double duration() {
        return mpv.getPropertyDouble("duration");
    }

    /** {@code track-list} em JSON. */
    String trackListJson() {
        return mpv.getPropertyString("track-list");
    }

    /** {@code chapter-list} em JSON. */
    String chapterListJson() {
        return mpv.getPropertyString("chapter-list");
    }

    /**
     * Uma linha para o logcat: decodificador em uso, formato e quadros perdidos
     * (no decodificador e na saída). Serve para saber por que o vídeo engasga.
     */
    String describePlayback() {
        return "hwdec=" + mpv.getPropertyString("hwdec-current")
                + " codec=" + mpv.getPropertyString("video-codec")
                + " " + mpv.getPropertyString("width") + "x" + mpv.getPropertyString("height")
                + " fps=" + mpv.getPropertyString("container-fps")
                + " saida=" + mpv.getPropertyString("estimated-vf-fps")
                + " perdidos(decodificador)=" + mpv.getPropertyString("decoder-frame-drop-count")
                + " perdidos(saida)=" + mpv.getPropertyString("frame-drop-count")
                + " cache=" + mpv.getPropertyString("demuxer-cache-duration") + "s";
    }

    // --- Superfície ---

    void attach(Surface surface) {
        mpv.attachSurface(surface);
        mpv.setOptionString("force-window", "yes");
        mpv.setPropertyString("vo", "gpu");
    }

    void resize(int width, int height) {
        mpv.setPropertyString("android-surface-size", width + "x" + height);
    }

    void detach() {
        mpv.setPropertyString("vo", "null");
        mpv.setOptionString("force-window", "no");
        mpv.detachSurface();
    }

    void destroy() {
        mpv.destroy();
    }
}
