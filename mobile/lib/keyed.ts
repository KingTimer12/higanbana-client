// Estado por chave: um sinal para cada chave, criado na primeira leitura ou escrita.
// Ler `get(k)` num efeito ou num componente só acompanha `k`; escrever uma chave
// não reavalia quem lê as outras (com um sinal só de dicionário, cada escrita
// copiava o dicionário inteiro e reavaliava todos os cards montados).
import { batch, type Signal, signal } from "nsh";

export class KeyedSignals<K, V> {
  private readonly slots = new Map<K, Signal<V | undefined>>();

  /** O valor da chave (acompanhado); `undefined` enquanto nada foi escrito nela. */
  get(key: K): V | undefined {
    return this.slot(key)();
  }

  set(key: K, value: V): void {
    this.slot(key).set(value);
  }

  /** Várias chaves de uma vez: quem lê mais de uma reavalia uma vez só. */
  setMany(entries: Iterable<[K, V]>): void {
    batch(() => {
      for (const [key, value] of entries) this.slot(key).set(value);
    });
  }

  private slot(key: K): Signal<V | undefined> {
    let slot = this.slots.get(key);
    if (!slot) {
      slot = signal<V | undefined>(undefined);
      this.slots.set(key, slot);
    }
    return slot;
  }
}
