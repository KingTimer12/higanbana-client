package dev.kingtimer12.higanbana.media;

import android.util.Log;

/**
 * Os decodificadores do aparelho, sondados em C++ pelo NDK MediaCodec
 * ({@code src/main/cpp/codec}). Os métodos {@code native} são ligados no
 * {@code JNI_OnLoad} da biblioteca.
 */
public final class NativeCodecs {
    private static final String TAG = "higanbana.codec";
    private static final boolean LOADED = load();

    private NativeCodecs() {}

    /** Se a biblioteca nativa carregou; sem ela os outros métodos devolvem {@code null}. */
    public static boolean available() {
        return LOADED;
    }

    /** O {@code hwdec} do mpv indicado para este aparelho, ou {@code null} sem a biblioteca. */
    public static String recommendedHwdec() {
        return LOADED ? nativeRecommendedHwdec() : null;
    }

    /** O decodificador que o sistema usaria para {@code mime} (ex.: {@code video/hevc}). */
    public static String decoderName(String mime) {
        return LOADED ? nativeDecoderName(mime) : null;
    }

    private static boolean load() {
        try {
            System.loadLibrary("higanbana_native");
            return true;
        } catch (UnsatisfiedLinkError e) {
            Log.w(TAG, "biblioteca nativa ausente", e);
            return false;
        }
    }

    private static native String nativeRecommendedHwdec();

    private static native String nativeDecoderName(String mime);
}
