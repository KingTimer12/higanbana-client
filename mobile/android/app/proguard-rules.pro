# Regras do R8 para quando isMinifyEnabled for ligado no release.

# Native Shell: o libnsh.so acha estas classes e membros pelo nome, via JNI.
-keep class com.longbridge.nsh.** { *; }

# libmpv: o libplayer.so chama de volta os métodos de MPVLib e dos observadores pelo nome.
-keep class dev.jdtech.mpv.** { *; }
-keep class * implements dev.jdtech.mpv.MPVLib$EventObserver { *; }

# C++: o JNI_OnLoad registra os métodos nativos de NativeCodecs pelo nome da classe e do método.
-keep class dev.kingtimer12.higanbana.media.NativeCodecs {
    native <methods>;
}

# Mantém linhas nos stack traces.
-keepattributes SourceFile,LineNumberTable
