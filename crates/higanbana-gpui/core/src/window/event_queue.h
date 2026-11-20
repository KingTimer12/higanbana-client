/* event_queue.h — fila circular fixa de eventos (sem alocação). */
#ifndef HG_EVENT_QUEUE_H
#define HG_EVENT_QUEUE_H
#include "hgui.h"

#define HG_EVENT_QUEUE_CAP 256

typedef struct hg_event_queue {
    hg_event items[HG_EVENT_QUEUE_CAP];
    uint32_t head, count;
} hg_event_queue;

/* Cheia: descarta o mais antigo (input recente vale mais). Movimentos de mouse
 * consecutivos são fundidos para não inundar a fila. */
void hg_queue_push(hg_event_queue* q, const hg_event* ev);
int  hg_queue_pop(hg_event_queue* q, hg_event* out);
#endif
