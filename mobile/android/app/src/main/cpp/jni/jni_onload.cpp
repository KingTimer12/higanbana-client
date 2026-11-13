// Entrada da biblioteca: registra os métodos nativos de cada classe Java pelo nome,
// em vez de depender de símbolos `Java_...` exportados.

#include <jni.h>

#include "jni/codecs_bridge.h"

extern "C" JNIEXPORT jint JNI_OnLoad(JavaVM* vm, void* /* reserved */) {
    JNIEnv* env = nullptr;
    if (vm->GetEnv(reinterpret_cast<void**>(&env), JNI_VERSION_1_6) != JNI_OK) return JNI_ERR;
    if (!higanbana::jni::registerCodecs(env)) return JNI_ERR;
    return JNI_VERSION_1_6;
}
