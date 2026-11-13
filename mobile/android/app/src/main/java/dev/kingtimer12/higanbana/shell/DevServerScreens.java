package dev.kingtimer12.higanbana.shell;

import android.net.Uri;
import android.util.Log;

import com.longbridge.nsh.NshView;

/** As telas servidas pelo {@code nsh dev --server-only}, com hot reload (via {@code adb reverse}). */
final class DevServerScreens implements ScreenSource {
    private static final String TAG = "higanbana";
    static final String DEFAULT_URL = "ws://127.0.0.1:8790/ws";

    private final String url;

    private DevServerScreens(String url) {
        this.url = url;
    }

    /** {@code url} só se apontar para o próprio aparelho (127.0.0.1/localhost); senão o padrão. */
    static DevServerScreens loopbackOnly(String url) {
        if (url == null) return new DevServerScreens(DEFAULT_URL);
        Uri uri = Uri.parse(url);
        String host = uri.getHost();
        boolean websocket = "ws".equals(uri.getScheme()) || "wss".equals(uri.getScheme());
        if (websocket && ("127.0.0.1".equals(host) || "localhost".equals(host))) return new DevServerScreens(url);
        Log.w(TAG, "servidor de telas recusado (só loopback): " + url);
        return new DevServerScreens(DEFAULT_URL);
    }

    @Override
    public void load(NshView view) {
        view.connectDevServer(url);
    }
}
