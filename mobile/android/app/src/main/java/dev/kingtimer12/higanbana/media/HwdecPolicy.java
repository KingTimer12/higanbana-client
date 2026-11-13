package dev.kingtimer12.higanbana.media;

import android.util.Log;

/** Qual decodificador de vídeo o mpv usa. */
public final class HwdecPolicy {
    private static final String TAG = "higanbana.codec";
    /** Hardware com os quadros copiados para a memória: o vo=gpu desenha as legendas por cima. */
    public static final String DEFAULT = "mediacodec-copy";

    private HwdecPolicy() {}

    /**
     * {@code override} quando dado (extra {@code hwdec} da intent, ex.: {@code no});
     * senão o que a sondagem nativa indicar; sem ela, {@link #DEFAULT}.
     */
    public static String resolve(String override) {
        if (override != null && !override.isEmpty()) {
            Log.i(TAG, "hwdec forçado: " + override);
            return override;
        }
        String recommended = NativeCodecs.recommendedHwdec();
        return recommended != null ? recommended : DEFAULT;
    }
}
