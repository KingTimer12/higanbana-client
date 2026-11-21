/* vk_instance.c — VkInstance única do processo, criada uma vez e nunca destruída.
 * Motivo: o driver AMD (amdvlk) compartilha estado D3D12 interno entre instâncias;
 * vkDestroyInstance numa thread derruba vkQueuePresentKHR/vkDestroyInstance em
 * outra (access violation em amdxc64.dll). Contextos em threads diferentes passam
 * a dividir a instância; cada um continua com seu próprio VkDevice. */
#include "vk_internal.h"
#include "error.h"

static INIT_ONCE  g_once = INIT_ONCE_STATIC_INIT;
static vk_api     g_api;      /* tabela de nível instância; somente leitura após init */
static VkInstance g_instance;

static BOOL CALLBACK init_instance(PINIT_ONCE once, PVOID param, PVOID* ctx) {
    (void)once; (void)param; (void)ctx;
    if (!vk_api_open(&g_api)) return TRUE;
    if (vk_instance_create(&g_api, &g_instance) != HG_OK) {
        vk_api_close(&g_api); /* sem instância: a DLL pode sair */
        g_instance = NULL;
    }
    return TRUE;
}

int vk_shared_instance(vk_api* api, VkInstance* out) {
    InitOnceExecuteOnce(&g_once, init_instance, NULL, NULL);
    if (!g_instance) return 0;
    *api = g_api; /* cópia: o device carrega suas funções nela */
    *out = g_instance;
    return 1;
}
