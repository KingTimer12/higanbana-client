#pragma once

#include <string>
#include <utility>

namespace higanbana::codec {

/** O decodificador que o sistema entrega para um tipo de vídeo. */
class DecoderInfo {
public:
    DecoderInfo() = default;
    DecoderInfo(std::string mime, std::string name) : mime_(std::move(mime)), name_(std::move(name)) {}

    const std::string& mime() const { return mime_; }
    /** Ex.: `c2.qti.avc.decoder`; vazio quando o aparelho não decodifica o tipo. */
    const std::string& name() const { return name_; }
    bool available() const { return !name_.empty(); }

    /** Implementação em software do próprio Android (`c2.android.*`, `OMX.google.*`). */
    bool isSoftware() const;
    /** Decodificador do emulador (`goldfish`/`ranchu`), que costuma travar com o mpv. */
    bool isEmulator() const;

private:
    std::string mime_;
    std::string name_;
};

}  // namespace higanbana::codec
