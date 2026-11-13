#include "codec/decoder_probe.h"

#include <android/log.h>
#include <media/NdkMediaCodec.h>

#include <memory>

namespace higanbana::codec {
namespace {

constexpr const char* kTag = "higanbana.codec";

/** Dono de um `AMediaCodec*`: libera ao sair do escopo. */
struct CodecDeleter {
    void operator()(AMediaCodec* codec) const { AMediaCodec_delete(codec); }
};
using CodecHandle = std::unique_ptr<AMediaCodec, CodecDeleter>;

}  // namespace

bool DecoderInfo::isSoftware() const {
    return name_.rfind("c2.android.", 0) == 0 || name_.rfind("OMX.google.", 0) == 0;
}

bool DecoderInfo::isEmulator() const {
    return name_.find("goldfish") != std::string::npos || name_.find("ranchu") != std::string::npos;
}

DecoderInfo DecoderProbe::probe(const std::string& mime) const {
    CodecHandle codec(AMediaCodec_createDecoderByType(mime.c_str()));
    if (!codec) {
        __android_log_print(ANDROID_LOG_INFO, kTag, "sem decodificador para %s", mime.c_str());
        return DecoderInfo(mime, "");
    }
    char* name = nullptr;
    if (AMediaCodec_getName(codec.get(), &name) != AMEDIA_OK || name == nullptr) {
        return DecoderInfo(mime, "");
    }
    DecoderInfo info(mime, name);
    AMediaCodec_releaseName(codec.get(), name);
    return info;
}

}  // namespace higanbana::codec
