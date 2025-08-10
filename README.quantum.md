# Quantum Mechanics Roadmap for TRIAD

## 1. Born Rule & Probabilistic Collapse
- [ ] Реализовать probabilistic sampling при коллапсе (measure):
  - Выбор состояния по вероятности (|amplitude|^2)
  - Использовать weighted random choice для superposition
- [ ] Добавить тесты для проверки корректности sampling

## 2. Advanced Decoherence
- [ ] Ввести decay factor, зависящий от времени и сетевых событий
- [ ] Моделировать потерю когерентности при network partition/merge
- [ ] Ввести параметры для разных типов decoherence (thermal, network, logical)

## 3. Phase Manipulation Protection
- [ ] Сделать phase content-based (hash(tx), time, nonce)
- [ ] Ввести randomization для phase при генерации волны
- [ ] Добавить проверки на аномалии фаз (anti-phase attacks)

## 4. Overlapping Transactions & Superposition
- [ ] Реализовать обработку overlapping tx с вероятностным разрешением конфликтов
- [ ] Ввести penalization для double-spend в superposition

## 5. Numerical Stability
- [ ] Внедрить Kahan summation для накопления амплитуд
- [ ] Добавить тесты на floating-point drift

---
**Ответственный:** Quantum Lead
**Ожидаемый результат:** Реалистичная квантовая модель, устойчивая к атакам и численным ошибкам.