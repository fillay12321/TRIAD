.PHONY: build run test clean run-simulator stress-test quantum-simulator quantum-stress-test

# Переменные
CARGO = cargo
BUILD_TYPE = release
ETH_ADDRESS ?= 0x1234567890abcdef1234567890abcdef12345678
PORT ?= 23000
DURATION ?= 20
CONSENSUS_NODES ?= 4
SHARDS ?= 2

# Основные команды
build:
	$(CARGO) build --$(BUILD_TYPE)

build-simulator:
	$(CARGO) build --$(BUILD_TYPE) --example ethereum_simulator

run:
	$(CARGO) run --$(BUILD_TYPE)

test:
	$(CARGO) test

clean:
	$(CARGO) clean
	rm -rf target

# Запуск эмулятора Ethereum
run-simulator:
	$(CARGO) run --$(BUILD_TYPE) --example ethereum_simulator -- $(ETH_ADDRESS) $(PORT)

# Запуск стресс-теста эмулятора с автоматической отправкой транзакций
stress-test:
	$(CARGO) run --$(BUILD_TYPE) --example ethereum_simulator -- $(ETH_ADDRESS) $(PORT) stress $(DURATION)

# Запуск квантового эмулятора Ethereum
quantum-simulator:
	$(CARGO) run --$(BUILD_TYPE) --example ethereum_simulator -- $(ETH_ADDRESS) $(PORT) quantum $(CONSENSUS_NODES) $(SHARDS)

# Запуск квантового стресс-теста эмулятора
quantum-stress-test:
	$(CARGO) run --$(BUILD_TYPE) --example ethereum_simulator -- $(ETH_ADDRESS) $(PORT) stress quantum $(CONSENSUS_NODES) $(SHARDS) $(DURATION)

# Дополнительные команды
help:
	@echo "Доступные команды:"
	@echo "  make build              - Собрать проект"
	@echo "  make build-simulator    - Собрать эмулятор Ethereum"
	@echo "  make run                - Запустить основной проект"
	@echo "  make test               - Запустить тесты"
	@echo "  make clean              - Очистить сборочные файлы"
	@echo "  make run-simulator      - Запустить эмулятор Ethereum"
	@echo "  make stress-test        - Запустить стресс-тест эмулятора с автоотправкой транзакций"
	@echo "  make quantum-simulator  - Запустить квантовый эмулятор Ethereum"
	@echo "  make quantum-stress-test - Запустить квантовый стресс-тест эмулятора"
	@echo ""
	@echo "Переменные:"
	@echo "  ETH_ADDRESS             - Ethereum адрес (по умолчанию: 0x1234567890abcdef1234567890abcdef12345678)"
	@echo "  PORT                    - Порт (по умолчанию: 23000)"
	@echo "  DURATION                - Длительность стресс-теста в секундах (по умолчанию: 20)"
	@echo "  CONSENSUS_NODES         - Число узлов квантового консенсуса (по умолчанию: 4)"
	@echo "  SHARDS                  - Число шардов (по умолчанию: 2)"
	@echo ""
	@echo "Примеры:"
	@echo "  make run-simulator ETH_ADDRESS=0xabcdef1234567890abcdef1234567890abcdef12 PORT=23001"
	@echo "  make stress-test DURATION=30"
	@echo "  make quantum-simulator CONSENSUS_NODES=8 SHARDS=4"
	@echo "  make quantum-stress-test CONSENSUS_NODES=8 SHARDS=4 DURATION=60" 