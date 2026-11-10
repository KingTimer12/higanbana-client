// Onde o vídeo toca. No Android é o libmpv (componente nativo `higanbana.mpv`,
// registrado pelo host em `android/`): abre mkv, legendas ASS e várias faixas.
// Nas outras plataformas é o `Video` do NSH, que toca o que o sistema toca e
// não tem faixas; serve para desenvolver as telas no desktop.
import { type NativeComponentRef, nativeComponent, platform, Video, type VideoRef } from "nsh";
import type { Chapter, Track } from "../lib/backend";

export interface VideoLoad {
  duration: number;
  tracks: Track[];
  chapters: Chapter[];
}

/** O que a tela do player comanda no vídeo. */
export interface VideoHandle {
  /** Pula para `seconds`. */
  seek(seconds: number): void;
}

export interface VideoSurfaceProps {
  /** O endereço do arquivo. */
  src: string;
  paused: boolean;
  /** Onde começar, em segundos (retomar um episódio). */
  startAt: number;
  /** A legenda: um id de faixa, `"off"`, ou `null` para a que o player escolher. */
  sub: number | "off" | null;
  /** A faixa de áudio, ou `null` para a que o player escolher. */
  audio: number | null;
  class?: string;
  /** O arquivo abriu: duração, faixas e capítulos. */
  onLoad?: (e: VideoLoad) => void;
  onProgress?: (e: { position: number; duration: number }) => void;
  onBuffer?: (e: { buffering: boolean }) => void;
  /** O arquivo chegou ao fim. */
  onEnd?: () => void;
  onError?: (e: { message: string }) => void;
  /** Recebe os comandos do vídeo quando ele existe. */
  handle?: (handle: VideoHandle) => void;
}

interface MpvProps {
  src: string;
  paused: boolean;
  startAt: number;
  /** Id da faixa, `"no"` (desligada) ou `"auto"`. */
  sub: string;
  /** Id da faixa ou `"auto"`. */
  audio: string;
}

interface MpvEvents {
  load: VideoLoad;
  progress: { position: number; duration: number };
  buffer: { buffering: boolean };
  end: Record<string, never>;
  error: { message: string };
}

const Mpv = nativeComponent<MpvProps, MpvEvents>("higanbana.mpv", {
  props: ["src", "paused", "startAt", "sub", "audio"],
  events: ["load", "progress", "buffer", "end", "error"],
});

/** Há libmpv nesta plataforma (faixas, legendas embutidas, mkv). */
export const hasMpv = platform.os === "android";

export function VideoSurface(props: VideoSurfaceProps) {
  if (hasMpv) {
    return (
      <Mpv
        ref={(node: NativeComponentRef) => props.handle?.({ seek: (to) => node.command("seek", { to }) })}
        src={props.src}
        paused={props.paused}
        startAt={props.startAt}
        sub={props.sub === null ? "auto" : props.sub === "off" ? "no" : String(props.sub)}
        audio={props.audio === null ? "auto" : String(props.audio)}
        class={props.class}
        onLoad={(e) => props.onLoad?.(e)}
        onProgress={(e) => props.onProgress?.(e)}
        onBuffer={(e) => props.onBuffer?.(e)}
        onEnd={() => props.onEnd?.()}
        onError={(e) => props.onError?.(e)}
      />
    );
  }

  let video: VideoRef | undefined;
  return (
    <Video
      ref={(node: VideoRef) => {
        video = node;
        props.handle?.({ seek: (to) => node.seek(to) });
      }}
      src={props.src}
      paused={props.paused}
      audioSession="playback"
      class={props.class}
      onLoad={(e) => {
        // O valor de quando abriu: a retomada acontece uma vez por arquivo.
        if (props.startAt > 0) video?.seek(props.startAt);
        props.onLoad?.({ duration: e.duration, tracks: [], chapters: [] });
      }}
      onProgress={(e) => props.onProgress?.({ position: e.currentTime, duration: e.duration })}
      onBuffer={(e) => props.onBuffer?.({ buffering: e.isBuffering })}
      onEnd={() => props.onEnd?.()}
      onError={(e) => props.onError?.({ message: e.message })}
    />
  );
}
