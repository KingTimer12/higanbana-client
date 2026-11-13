package dev.kingtimer12.higanbana.shell;

import android.content.res.AssetManager;
import android.util.Log;

import com.longbridge.nsh.NshView;

import java.io.IOException;
import java.io.InputStream;

/** As telas empacotadas no APK, em {@code assets/nsh} (geradas pela tarefa {@code nshBundle}). */
final class AssetScreens implements ScreenSource {
    private static final String TAG = "higanbana";
    static final String DIR = "nsh";

    private final AssetManager assets;

    AssetScreens(AssetManager assets) {
        this.assets = assets;
    }

    @Override
    public void load(NshView view) {
        if (!isPresent()) Log.e(TAG, "bundle ausente em assets/" + DIR + " (a tarefa nshBundle rodou?)");
        view.loadAssetBundle(DIR);
    }

    private boolean isPresent() {
        try (InputStream in = assets.open(DIR + "/bundle.nshc")) {
            return true;
        } catch (IOException e) {
            return false;
        }
    }
}
