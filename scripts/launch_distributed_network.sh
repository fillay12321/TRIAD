#!/bin/bash

# 🌐 TRIAD Distributed Network Launcher via GitHub Codespaces
# Запускает распределенную сеть из 10 узлов через реальные GitHub Codespaces

set -e

echo "🌐 Запуск TRIAD Distributed Network через GitHub Codespaces..."
echo "================================================================"

# Цвета для вывода
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Загружаем конфигурацию GitHub API
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/github_config.sh"

# Конфигурация узлов сети (совместимость с bash 3.2)
NODE_IDS=("triad-node-0" "triad-node-1" "triad-node-2" "triad-node-3" "triad-node-4" "triad-node-5" "triad-node-6" "triad-node-7" "triad-node-8" "triad-node-9")
NODE_CONFIGS=("8080:0:WestUs2" "8081:1:EastUs" "8082:2:WestEurope" "8083:3:SoutheastAsia" "8084:4:WestUs2" "8085:0:EastUs" "8086:1:WestEurope" "8087:2:SoutheastAsia" "8088:3:WestUs2" "8089:4:EastUs")

# Функция для получения конфигурации узла
get_node_config() {
    local node_id="$1"
    local i=0
    for id in "${NODE_IDS[@]}"; do
        if [ "$id" = "$node_id" ]; then
            echo "${NODE_CONFIGS[$i]}"
            return 0
        fi
        i=$((i + 1))
    done
    return 1
}

# Статистика
total_nodes=0
created_nodes=0
failed_nodes=0
active_nodes=0

# Функция для создания и настройки Codespace
setup_codespace() {
    local node_id="$1"
    local port="$2"
    local shard_id="$3"
    local location="$4"
    
    echo -e "${BLUE}🚀 Настройка Codespace для ${node_id}...${NC}"
    echo "  📍 Порт: ${port}"
    echo "  🔢 Shard: ${shard_id}"
    echo "  🌍 Локация: ${location}"
    
    # Временно меняем локацию для этого узла
    local original_location="$CODESPACE_LOCATION"
    export CODESPACE_LOCATION="$location"
    
    # Создаем Codespace
    if create_codespace "$node_id" "$port" "$shard_id"; then
        created_nodes=$((created_nodes + 1))
        
        # Ждем, пока Codespace запустится
        local codespace_id=$(cat "logs/${node_id}_codespace_id")
        echo -e "${CYAN}⏳ Ожидаем запуска Codespace ${codespace_id}...${NC}"
        
        # Ждем статуса "Available"
        local max_wait=300  # 5 минут
        local wait_time=0
        
        while [ $wait_time -lt $max_wait ]; do
            local status=$(get_codespace_status "$codespace_id")
            echo -e "  📊 Статус: ${status}"
            
            if [ "$status" = "Available" ]; then
                echo -e "${GREEN}✅ Codespace ${codespace_id} готов к работе!${NC}"
                break
            elif [ "$status" = "Failed" ] || [ "$status" = "Canceled" ]; then
                echo -e "${RED}❌ Codespace ${codespace_id} не удалось запустить${NC}"
                failed_nodes=$((failed_nodes + 1))
                return 1
            fi
            
            sleep 10
            wait_time=$((wait_time + 10))
        done
        
        if [ $wait_time -ge $max_wait ]; then
            echo -e "${RED}❌ Таймаут ожидания Codespace ${codespace_id}${NC}"
            failed_nodes=$((failed_nodes + 1))
            return 1
        fi
        
        # Настраиваем узел в Codespace
        if setup_node_in_codespace "$codespace_id" "$node_id" "$port" "$shard_id"; then
            active_nodes=$((active_nodes + 1))
            echo -e "${GREEN}✅ Узел ${node_id} успешно настроен!${NC}"
        else
            echo -e "${RED}❌ Не удалось настроить узел ${node_id}${NC}"
            failed_nodes=$((failed_nodes + 1))
            return 1
        fi
        
    else
        echo -e "${RED}❌ Не удалось создать Codespace для ${node_id}${NC}"
        failed_nodes=$((failed_nodes + 1))
        return 1
    fi
    
    # Восстанавливаем оригинальную локацию
    export CODESPACE_LOCATION="$original_location"
}

# Функция для настройки узла в Codespace
setup_node_in_codespace() {
    local codespace_id="$1"
    local node_id="$2"
    local port="$3"
    local shard_id="$4"
    
    echo -e "${PURPLE}🔧 Настройка узла ${node_id} в Codespace ${codespace_id}...${NC}"
    
    # Команды для настройки узла
    local setup_commands=(
        "cd /workspaces/TRIAD"
        "cargo build --release"
        "mkdir -p logs"
        "echo 'Узел ${node_id} готов к работе' > logs/node_${node_id}.log"
    )
    
    # Выполняем команды настройки
    for cmd in "${setup_commands[@]}"; do
        echo "  🔧 Выполняем: $cmd"
        if ! run_in_codespace "$codespace_id" "$cmd"; then
            echo -e "  ❌ Ошибка выполнения: $cmd"
            return 1
        fi
    done
    
    # Запускаем узел в фоне
    local run_cmd="RUST_LOG=info cargo run --example quantum_consensus_demo -- --node-id ${node_id} --port ${port} --shard-id ${shard_id} > logs/node_${node_id}.log 2>&1 &"
    
    echo "  🚀 Запускаем узел: $run_cmd"
    if run_in_codespace "$codespace_id" "$run_cmd"; then
        echo -e "  ✅ Узел ${node_id} запущен"
        
        # Сохраняем информацию о запущенном узле
        echo "$codespace_id:$port:$shard_id" > "logs/${node_id}_active"
        return 0
    else
        echo -e "  ❌ Не удалось запустить узел ${node_id}"
        return 1
    fi
}

# Функция для проверки статуса узла
check_node_status() {
    local node_id="$1"
    local port="$2"
    local shard_id="$3"
    
    local active_file="logs/${node_id}_active"
    if [ -f "$active_file" ]; then
        IFS=':' read -r codespace_id node_port node_shard <<< "$(cat "$active_file")"
        
        if [ "$node_port" = "$port" ] && [ "$node_shard" = "$shard_id" ]; then
            echo -e "${GREEN}✅ ${node_id}: ACTIVE (Codespace: ${codespace_id})${NC}"
            return 0
        fi
    fi
    
    echo -e "${RED}❌ ${node_id}: INACTIVE${NC}"
    return 1
}

# Функция для остановки всех Codespaces
stop_all_codespaces() {
    echo -e "${YELLOW}🛑 Останавливаем все Codespaces...${NC}"
    
    for node_id in "${NODE_IDS[@]}"; do
        local active_file="logs/${node_id}_active"
        if [ -f "$active_file" ]; then
            IFS=':' read -r codespace_id node_port node_shard <<< "$(cat "$active_file")"
            
            echo -e "${BLUE}🛑 Останавливаем Codespace ${codespace_id} для ${node_id}...${NC}"
            if stop_codespace "$codespace_id"; then
                echo -e "${GREEN}✅ Codespace ${codespace_id} остановлен${NC}"
            else
                echo -e "${RED}❌ Не удалось остановить Codespace ${codespace_id}${NC}"
            fi
            
            rm -f "$active_file"
        fi
    done
    
    echo -e "${GREEN}✅ Все Codespaces остановлены${NC}"
}

# Обработка сигналов для корректного завершения
trap stop_all_codespaces SIGINT SIGTERM

# Основная логика
main() {
    echo -e "${BLUE}🔧 Инициализация GitHub API...${NC}"
    init_github_api
    
    echo ""
    echo -e "${BLUE}📋 Конфигурация распределенной сети:${NC}"
    for node_id in "${NODE_IDS[@]}"; do
        IFS=':' read -r port shard_id location <<< "$(get_node_config "$node_id")"
        echo -e "  ${node_id}: порт ${port}, shard ${shard_id}, локация ${location}"
        total_nodes=$((total_nodes + 1))
    done
    echo ""
    
    echo -e "${BLUE}🚀 Создание и настройка Codespaces...${NC}"
    echo "Это может занять несколько минут..."
    echo ""
    
    # Создаем и настраиваем все Codespaces
    for node_id in "${NODE_IDS[@]}"; do
        IFS=':' read -r port shard_id location <<< "$(get_node_config "$node_id")"
        
        if setup_codespace "$node_id" "$port" "$shard_id" "$location"; then
            echo -e "${GREEN}✅ ${node_id} успешно настроен${NC}"
        else
            echo -e "${RED}❌ ${node_id} не удалось настроить${NC}"
        fi
        
        echo ""
        sleep 5  # Небольшая пауза между созданием Codespaces
    done
    
    echo ""
    echo -e "${BLUE}📊 Статистика создания сети:${NC}"
    echo -e "  Всего узлов: ${total_nodes}"
    echo -e "  Успешно создано: ${GREEN}${created_nodes}${NC}"
    echo -e "  Активных узлов: ${GREEN}${active_nodes}${NC}"
    echo -e "  Ошибок: ${RED}${failed_nodes}${NC}"
    
    echo ""
    echo -e "${BLUE}🔍 Проверка статуса сети...${NC}"
    for node_id in "${NODE_IDS[@]}"; do
        IFS=':' read -r port shard_id location <<< "$(get_node_config "$node_id")"
        check_node_status "$node_id" "$port" "$shard_id"
    done
    
    echo ""
    if [ $failed_nodes -eq 0 ]; then
        echo -e "${GREEN}🎉 Все узлы успешно запущены! Распределенная сеть готова к работе!${NC}"
        echo ""
        echo -e "${YELLOW}💡 Команды для управления:${NC}"
        echo "  Проверить статус: ./scripts/check_distributed_network.sh"
        echo "  Остановить сеть:  ./scripts/stop_distributed_network.sh"
        echo "  Просмотр логов:    ./scripts/view_codespace_logs.sh"
    else
        echo -e "${YELLOW}⚠️  ${failed_nodes} узлов не удалось запустить. Проверьте логи.${NC}"
    fi
    
    echo "================================================================"
}

# Запуск основной логики
main "$@"
