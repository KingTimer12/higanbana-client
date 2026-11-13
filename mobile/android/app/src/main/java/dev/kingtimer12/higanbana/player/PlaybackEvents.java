package dev.kingtimer12.higanbana.player;

import android.os.SystemClock;

import dev.jdtech.mpv.MPVLib;

/**
 * Os eventos do mpv (na thread de eventos dele) traduzidos para a tela: abriu,
 * progresso, carregando, fim e erro.
 */
final class PlaybackEvents implements MPVLib.EventObserver {
    private static final long PROGRESS_EVERY_MS = 250;
    private static final String OPEN_FAILED = "O arquivo não abriu. Confira a conexão e tente de novo.";

    private final MpvEngine mpv;
    private final PlaybackState state;
    private final PlayerEmitter emit;
    private long progressAt;

    PlaybackEvents(MpvEngine mpv, PlaybackState state, PlayerEmitter emit) {
        this.mpv = mpv;
        this.state = state;
        this.emit = emit;
    }

    @Override
    public void eventProperty(String property, double value) {
        if (state.destroyed) return;
        if ("duration".equals(property)) {
            state.duration = value;
        } else if ("time-pos".equals(property)) {
            long now = SystemClock.elapsedRealtime();
            if (now - progressAt < PROGRESS_EVERY_MS) return;
            progressAt = now;
            emit.progress(value, state.duration);
        }
    }

    @Override
    public void eventProperty(String property, boolean value) {
        if (state.destroyed) return;
        if ("paused-for-cache".equals(property)) {
            emit.buffering(value);
        } else if ("eof-reached".equals(property) && value && state.loaded) {
            emit.ended();
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
        if (state.destroyed) return;
        if (id == MPVLib.MpvEvent.MPV_EVENT_START_FILE) {
            state.loaded = false;
        } else if (id == MPVLib.MpvEvent.MPV_EVENT_FILE_LOADED) {
            fileLoaded();
        } else if (id == MPVLib.MpvEvent.MPV_EVENT_END_FILE && !state.loaded) {
            // Terminou sem ter aberto: o arquivo não existe, a rede caiu ou o formato não abre.
            emit.error(OPEN_FAILED);
        }
    }

    private void fileLoaded() {
        state.loaded = true;
        mpv.forgetStart();
        if (!"auto".equals(state.sub)) mpv.selectSubtitle(state.sub);
        if (!"auto".equals(state.audio)) mpv.selectAudio(state.audio);
        Double length = mpv.duration();
        if (length != null) state.duration = length;
        emit.loaded(state.duration, MediaInfo.tracks(mpv.trackListJson()), MediaInfo.chapters(mpv.chapterListJson()));
    }
}
