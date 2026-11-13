#include "codec/hwdec_policy.h"

#include <android/log.h>

#include <mutex>

namespace higanbana::codec {
namespace {

constexpr const char* kTag = "higanbana.codec";
constexpr const char* kAvc = "video/avc";

}  // namespace

std::string HwdecPolicy::recommend() const {
    static std::once_flag once;
    static std::string cached;
    std::call_once(once, [this] { cached = decide(); });
    return cached;
}

std::string HwdecPolicy::decide() const {
    const DecoderInfo avc = probe_.probe(kAvc);
    const char* choice = kHardware;
    if (!avc.available() || avc.isEmulator() || avc.isSoftware()) choice = kSoftware;
    __android_log_print(ANDROID_LOG_INFO, kTag, "decodificador %s: '%s' -> hwdec=%s",
                        kAvc, avc.name().c_str(), choice);
    return choice;
}

}  // namespace higanbana::codec
