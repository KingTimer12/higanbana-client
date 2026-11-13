#pragma once

#include <string>

#include "codec/decoder_probe.h"

namespace higanbana::codec {

/**
 * Escolhe o `hwdec` do mpv a partir dos decodificadores do aparelho:
 *
 * - `mediacodec-copy` quando há decodificador de hardware de verdade para H.264;
 * - `no` (o ffmpeg do mpv, em software) no emulador, cujo decodificador costuma não
 *   responder, e quando o MediaCodec só oferece o decodificador em software do Android,
 *   que não ganha nada do ffmpeg e ainda copia os quadros.
 *
 * A sondagem roda uma vez por processo; as chamadas seguintes devolvem o mesmo valor.
 */
class HwdecPolicy {
public:
    explicit HwdecPolicy(DecoderProbe probe = DecoderProbe()) : probe_(probe) {}

    std::string recommend() const;

    static constexpr const char* kHardware = "mediacodec-copy";
    static constexpr const char* kSoftware = "no";

private:
    std::string decide() const;

    DecoderProbe probe_;
};

}  // namespace higanbana::codec
