package dev.kingtimer12.higanbana.player;

/**
 * O que a thread da interface, a de controle e a de eventos do mpv compartilham.
 * Cada campo tem um só escritor (indicado ao lado); os outros só leem.
 */
final class PlaybackState {
    /** Interface ({@link MpvPlayerComponent#destroy}). */
    volatile boolean destroyed;
    /** Eventos do mpv: o arquivo atual terminou de abrir. */
    volatile boolean loaded;
    /** Eventos do mpv, em segundos. */
    volatile double duration;
    /** Controle: faixa de legenda pedida ({@code "auto"}, {@code "no"} ou o id). */
    volatile String sub = "auto";
    /** Controle: faixa de áudio pedida. */
    volatile String audio = "auto";
}
