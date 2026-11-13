#pragma once

#include <jni.h>

namespace higanbana::jni {

/** Liga os métodos `native` de `dev.kingtimer12.higanbana.media.NativeCodecs`. */
bool registerCodecs(JNIEnv* env);

}  // namespace higanbana::jni
