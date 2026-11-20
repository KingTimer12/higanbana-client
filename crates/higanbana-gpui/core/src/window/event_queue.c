#include "event_queue.h"

static hg_event* at(hg_event_queue* q, uint32_t i) { return &q->items[(q->head + i) % HG_EVENT_QUEUE_CAP]; }

void hg_queue_push(hg_event_queue* q, const hg_event* ev) {
    if (ev->kind == HG_EVENT_MOUSE_MOVE && q->count > 0) {
        hg_event* last = at(q, q->count - 1);
        if (last->kind == HG_EVENT_MOUSE_MOVE) { *last = *ev; return; }
    }
    if (q->count == HG_EVENT_QUEUE_CAP) {
        q->head = (q->head + 1) % HG_EVENT_QUEUE_CAP;
        q->count--;
    }
    *at(q, q->count++) = *ev;
}

int hg_queue_pop(hg_event_queue* q, hg_event* out) {
    if (q->count == 0) return 0;
    *out = *at(q, 0);
    q->head = (q->head + 1) % HG_EVENT_QUEUE_CAP;
    q->count--;
    return 1;
}
