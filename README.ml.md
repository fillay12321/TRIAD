# AI/ML Roadmap for TRIAD

## 1. Feature Engineering
- [ ] Собрать расширенный набор метрик: latency, throughput, interference quality, node uptime, shard load
- [ ] Ввести автоматический feature selection

## 2. Online Learning
- [ ] Реализовать online learning pipeline для моделей ошибок и шардинга
- [ ] Ввести автоматический retrain при изменении метрик

## 3. Dynamic Sharding
- [ ] ML-based triggers для resharding (load, latency, interference)
- [ ] Миграция states между шардами с помощью ML
- [ ] Решение cold start через transfer learning

## 4. Model Serving
- [ ] Внедрить distributed model serving (на каждом узле)
- [ ] Добавить мониторинг качества моделей

---
**Ответственный:** ML Team
**Ожидаемый результат:** Адаптивная, самообучающаяся сеть с оптимальным шардингом и предсказанием ошибок.