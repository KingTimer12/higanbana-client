/* gl_buffers.c — instance VBO reaproveitado entre frames: órfão a cada frame,
 * cresce em potência de 2. */
#include "gl_internal.h"
#include "text.h"

void gl_buffer_orphan(const gl_api* gl, gl_buffer* b, uint32_t bytes) {
    if (!b->name) gl->GenBuffers(1, &b->name);
    uint32_t cap = b->capacity ? b->capacity : 64 * 1024;
    while (cap < bytes) cap *= 2;
    b->capacity = cap;
    gl->BindBuffer(GL_ARRAY_BUFFER, b->name);
    gl->BufferData(GL_ARRAY_BUFFER, (GLsizeiptr)cap, NULL, GL_STREAM_DRAW);
}

void gl_buffer_release(const gl_api* gl, gl_buffer* b) {
    if (b->name) gl->DeleteBuffers(1, &b->name);
    b->name = 0;
    b->capacity = 0;
}

uint64_t gl_gpu_bytes(const gl_device* d) {
    uint64_t bytes = (uint64_t)d->quads.capacity + d->glyphs.instances.capacity;
    bytes += (uint64_t)d->images.instances.capacity + d->images.texture_bytes;
    if (d->glyphs.atlas) bytes += (uint64_t)HG_GLYPH_ATLAS_SIZE * HG_GLYPH_ATLAS_SIZE; /* R8 */
    return bytes;
}
