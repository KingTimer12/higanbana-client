#include "jni/codecs_bridge.h"

#include "codec/decoder_probe.h"
#include "codec/hwdec_policy.h"
#include "jni/scoped_utf_chars.h"

namespace higanbana::jni {
namespace {

constexpr const char* kClass = "dev/kingtimer12/higanbana/media/NativeCodecs";

jstring recommendedHwdec(JNIEnv* env, jclass) {
    const std::string hwdec = codec::HwdecPolicy().recommend();
    return env->NewStringUTF(hwdec.c_str());
}

jstring decoderName(JNIEnv* env, jclass, jstring mime) {
    ScopedUtfChars type(env, mime);
    if (!type.valid()) return nullptr;
    const codec::DecoderInfo info = codec::DecoderProbe().probe(type.str());
    return info.available() ? env->NewStringUTF(info.name().c_str()) : nullptr;
}

const JNINativeMethod kMethods[] = {
    {"nativeRecommendedHwdec", "()Ljava/lang/String;", reinterpret_cast<void*>(recommendedHwdec)},
    {"nativeDecoderName", "(Ljava/lang/String;)Ljava/lang/String;", reinterpret_cast<void*>(decoderName)},
};

}  // namespace

bool registerCodecs(JNIEnv* env) {
    jclass cls = env->FindClass(kClass);
    if (cls == nullptr) return false;
    const jint count = static_cast<jint>(sizeof(kMethods) / sizeof(kMethods[0]));
    const bool ok = env->RegisterNatives(cls, kMethods, count) == JNI_OK;
    env->DeleteLocalRef(cls);
    return ok;
}

}  // namespace higanbana::jni
