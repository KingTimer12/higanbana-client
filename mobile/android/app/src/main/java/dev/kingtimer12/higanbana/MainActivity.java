package dev.kingtimer12.higanbana;

import android.app.Activity;
import android.os.Bundle;

import dev.kingtimer12.higanbana.media.HwdecPolicy;
import dev.kingtimer12.higanbana.player.MpvPlayerComponent;
import dev.kingtimer12.higanbana.shell.NshHost;
import dev.kingtimer12.higanbana.shell.ScreenSource;
import dev.kingtimer12.higanbana.shell.SystemBars;

/**
 * A atividade do app: uma {@code NshView} em tela cheia com o player libmpv
 * registrado como componente nativo ({@code higanbana.mpv}).
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
        host.show(ScreenSource.choose(this, getIntent().getStringExtra("url")));
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
