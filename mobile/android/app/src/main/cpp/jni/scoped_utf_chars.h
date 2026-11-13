#pragma once

#include <jni.h>

#include <string>

namespace higanbana::jni {

/** Os bytes UTF-8 modificados de uma `jstring`, liberados ao sair do escopo. */
class ScopedUtfChars {
public:
    ScopedUtfChars(JNIEnv* env, jstring value)
        : env_(env), value_(value), chars_(value ? env->GetStringUTFChars(value, nullptr) : nullptr) {}
    ~ScopedUtfChars() {
        if (chars_) env_->ReleaseStringUTFChars(value_, chars_);
    }
    ScopedUtfChars(const ScopedUtfChars&) = delete;
    ScopedUtfChars& operator=(const ScopedUtfChars&) = delete;

    bool valid() const { return chars_ != nullptr; }
    std::string str() const { return chars_ ? std::string(chars_) : std::string(); }

private:
    JNIEnv* env_;
    jstring value_;
    const char* chars_;
};

}  // namespace higanbana::jni
