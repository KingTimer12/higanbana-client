package dev.kingtimer12.higanbana.update;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.net.Uri;
import android.util.Log;

import androidx.core.content.FileProvider;

import java.io.File;
import java.util.regex.Pattern;

/**
 * Abre o instalador de pacotes com o APK de uma atualização.
 *
 * O núcleo baixa e confere o APK (SHA-256 do {@code SHA256SUMS} da release) em
 * {@code <files>/nsh/<pacote>/cache/update/}; a tela chama {@code openUrl} com
 * {@code higanbana://update/install?file=<nome>}, e este handler troca o endereço
 * pelo instalador. Só aceita um nome de APK do app, dentro dessa pasta.
 */
public final class UpdateInstaller {
    private static final String TAG = "higanbana.update";
    private static final Pattern FILE = Pattern.compile("Higanbana-\\d+\\.\\d+\\.\\d+-android\\.apk");
    private static final String APK = "application/vnd.android.package-archive";

    private final Activity activity;

    public UpdateInstaller(Activity activity) {
        this.activity = activity;
    }

    /** Se {@code url} é um pedido de instalação, abre o instalador e diz se abriu. {@code null}: não é. */
    public Boolean handle(String url) {
        Uri uri = Uri.parse(url);
        if (!"higanbana".equals(uri.getScheme()) || !"update".equals(uri.getHost()) || !"/install".equals(uri.getPath())) {
            return null;
        }
        String name = uri.getQueryParameter("file");
        if (name == null || !FILE.matcher(name).matches()) return false;

        String pkg = activity.getPackageName();
        File apk = new File(activity.getFilesDir(), "nsh/" + pkg + "/cache/update/" + name);
        if (!apk.isFile()) return false;
        try {
            Uri content = FileProvider.getUriForFile(activity, pkg + ".updates", apk);
            Intent install = new Intent(Intent.ACTION_VIEW)
                .setDataAndType(content, APK)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            activity.startActivity(install);
            return true;
        } catch (IllegalArgumentException | ActivityNotFoundException | SecurityException e) {
            Log.e(TAG, "não foi possível abrir o instalador", e);
            return false;
        }
    }
}
