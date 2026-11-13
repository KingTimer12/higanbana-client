package dev.kingtimer12.higanbana.player;

import android.content.Context;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;

/** A {@link SurfaceView} onde o mpv desenha; repassa o ciclo de vida dela ao controle. */
final class VideoSurface implements SurfaceHolder.Callback {
    private final SurfaceView view;
    private final PlaybackController controller;
    private final PlaybackState state;

    VideoSurface(Context context, PlaybackController controller, PlaybackState state) {
        this.controller = controller;
        this.state = state;
        view = new SurfaceView(context);
        view.getHolder().addCallback(this);
    }

    View view() {
        return view;
    }

    /** Roda {@code action} na thread da interface, depois do que já está na fila dela. */
    void post(Runnable action) {
        view.post(action);
    }

    void release() {
        view.getHolder().removeCallback(this);
    }

    @Override
    public void surfaceCreated(SurfaceHolder holder) {
        if (!state.destroyed) controller.surfaceCreated(holder.getSurface());
    }

    @Override
    public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        if (!state.destroyed) controller.surfaceResized(width, height);
    }

    @Override
    public void surfaceDestroyed(SurfaceHolder holder) {
        if (!state.destroyed) controller.surfaceDestroyed();
    }
}
