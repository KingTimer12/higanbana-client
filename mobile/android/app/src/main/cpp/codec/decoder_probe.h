#pragma once

#include <string>

#include "codec/decoder_info.h"

namespace higanbana::codec {

/**
 * Pergunta ao MediaCodec qual decodificador ele usaria para um tipo de vídeo, criando
 * e liberando uma instância (não decodifica nada).
 */
class DecoderProbe {
public:
    DecoderInfo probe(const std::string& mime) const;
};

}  // namespace higanbana::codec
