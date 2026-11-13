package dev.kingtimer12.higanbana.shell;

import android.content.Context;

import com.longbridge.nsh.NshView;

import dev.kingtimer12.higanbana.BuildConfig;

/** De onde a {@link NshView} tira as telas. */
public interface ScreenSource {
    void load(NshView view);

    /**
     * Flavor {@code bundled}: sempre o bundle do APK. Flavor {@code devServer}: o servidor
     * do {@code nsh dev}, em {@code devUrl} se for de loopback, senão no endereço padrão.
     *
     * A atividade é exportada, então o extra {@code url} vem de qualquer app: nunca escolhe
     * a origem das telas fora do flavor de desenvolvimento.
     */
    static ScreenSource choose(Context context, String devUrl) {
        if (BuildConfig.DEV_SERVER) return DevServerScreens.loopbackOnly(devUrl);
        return new AssetScreens(context.getAssets());
    }
}
