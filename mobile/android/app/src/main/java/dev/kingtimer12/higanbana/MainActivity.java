package dev.kingtimer12.higanbana;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;

import dev.kingtimer12.higanbana.media.HwdecPolicy;
import dev.kingtimer12.higanbana.player.MpvPlayerComponent;
import dev.kingtimer12.higanbana.shell.NshHost;
import dev.kingtimer12.higanbana.shell.ScreenSource;
import dev.kingtimer12.higanbana.shell.SystemBars;
import dev.kingtimer12.higanbana.update.UpdateInstaller;

/**
 * A atividade do app: uma {@code NshView} em tela cheia com o player libmpv
 * registrado como componente nativo ({@code higanbana.mpv}).
 *
 * O {@code openUrl} das telas passa primeiro pelo {@link UpdateInstaller}
 * ({@code higanbana://update/install}); o resto segue para o sistema.
 *
 * Extras da intent: {@code hwdec} (força o decodificador do mpv, ex.: {@code no}) e
 * {@code url} (outro servidor do {@code nsh dev}).
 */
public final class MainActivity extends Activity {
    private NshHost host;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        new SystemBars(getWindow()).drawBehind();

        String hwdec = HwdecPolicy.resolve(getIntent().getStringExtra("hwdec"));
        host = new NshHost(this);
        host.register(MpvPlayerComponent.NAME, context -> new MpvPlayerComponent(this, context, hwdec));
        UpdateInstaller updates = new UpdateInstaller(this);
        host.onOpenUrl(url -> {
            Boolean installed = updates.handle(url);
            return installed != null ? installed : openOutside(url);
        });
        host.show(ScreenSource.choose(this, getIntent().getStringExtra("url")));
    }

    /** O que o NSH faria sem o handler: a URL em outro app (navegador, Discord...). */
    private boolean openOutside(String url) {
        try {
            startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            return true;
        } catch (ActivityNotFoundException e) {
            return false;
        }
    }

    /** Android 12 e anteriores: o app recebe o botão voltar primeiro. */
    @Override
    @SuppressWarnings("deprecation")
    public void onBackPressed() {
        if (host == null || !host.handleBack()) super.onBackPressed();
    }

    @Override
    protected void onDestroy() {
        if (host != null) host.destroy();
        super.onDestroy();
    }
}
