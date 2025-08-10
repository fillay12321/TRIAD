#!/bin/bash

# 🛑 TRIAD Distributed Network Stopper
# Останавливает все узлы в распределенной сети через GitHub Codespaces

set -e

echo "🛑 Остановка TRIAD Distributed Network..."
echo "========================================="

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

# Конфигурация узлов сети
declare -A NODES
NODES["triad-node-0"]="8080:0:WestUs2"
NODES["triad-node-1"]="8081:1:EastUs"
NODES["triad-node-2"]="8082:2:WestEurope"
NODES["triad-node-3"]="8083:3:SoutheastAsia"
NODES["triad-node-4"]="8084:4:WestUs2"
NODES["triad-node-5"]="8085:0:EastUs"
NODES["triad-node-6"]="8086:1:WestEurope"
NODES["triad-node-7"]="8087:2:SoutheastAsia"
NODES["triad-node-8"]="8088:3:WestUs2"
NODES["triad-node-9"]="8089:4:EastUs"

# Статистика
total_nodes=0
stopped_nodes=0
failed_stops=0
deleted_nodes=0

# Функция для остановки узла
stop_node() {
    local node_id="$1"
    local port="$2"
    local shard_id="$3"
    
    echo -e "${BLUE}🛑 Останавливаем ${node_id}...${NC}"
    echo "  📍 Порт: ${port}"
    echo "  🔢 Shard: ${shard_id}"
    
    local active_file="logs/${node_id}_active"
    if [ -f "$active_file" ]; then
        IFS=':' read -r codespace_id node_port node_shard <<< "$(cat "$active_file")"
        
        if [ "$node_port" = "$port" ] && [ "$node_shard" = "$shard_id" ]; then
            echo -e "  🆔 Codespace ID: ${codespace_id}"
            
            # Сначала останавливаем процесс узла
            echo -e "  🔄 Останавливаем процесс узла..."
            local stop_process_cmd="pkill -f 'quantum_consensus_demo.*${node_id}' || true"
            if run_in_codespace "$codespace_id" "$stop_process_cmd"; then
                echo -e "  ✅ Процесс узла остановлен"
            else
                echo -e "  ⚠️  Процесс узла не найден или уже остановлен"
            fi
            
            # Ждем немного для корректного завершения
            sleep 2
            
            # Проверяем, что процесс действительно остановлен
            local process_check_cmd="ps aux | grep 'quantum_consensus_demo.*${node_id}' | grep -v grep | wc -l"
            local process_count=$(run_in_codespace "$codespace_id" "$process_check_cmd")
            
            if [ "$process_count" = "0" ]; then
                echo -e "  ✅ Процесс узла полностью остановлен"
                stopped_nodes=$((stopped_nodes + 1))
            else
                echo -e "  ⚠️  Процесс узла все еще работает (${process_count} процессов)"
                # Принудительно останавливаем
                local force_stop_cmd="pkill -9 -f 'quantum_consensus_demo.*${node_id}' || true"
                run_in_codespace "$codespace_id" "$force_stop_cmd"
                echo -e "  ✅ Процесс узла принудительно остановлен"
                stopped_nodes=$((stopped_nodes + 1))
            fi
            
            # Останавливаем Codespace
            echo -e "  🛑 Останавливаем Codespace ${codespace_id}..."
            if stop_codespace "$codespace_id"; then
                echo -e "  ✅ Codespace ${codespace_id} остановлен"
            else
                echo -e "  ❌ Не удалось остановить Codespace ${codespace_id}"
                failed_stops=$((failed_stops + 1))
            fi
            
            # Удаляем файл активности
            rm -f "$active_file"
            echo -e "  🗑️  Файл активности удален"
            
        else
            echo -e "  ❌ Несоответствие конфигурации"
            failed_stops=$((failed_stops + 1))
        fi
    else
        echo -e "  ⚠️  Узел не активен"
    fi
    
    echo ""
}

# Функция для удаления Codespace
delete_codespace() {
    local node_id="$1"
    local codespace_id="$2"
    
    echo -e "${PURPLE}🗑️  Удаляем Codespace ${codespace_id} для ${node_id}...${NC}"
    
    if delete_codespace "$codespace_id"; then
        echo -e "  ✅ Codespace ${codespace_id} удален"
        deleted_nodes=$((deleted_nodes + 1))
        
        # Удаляем файл с ID Codespace
        local codespace_id_file="logs/${node_id}_codespace_id"
        if [ -f "$codespace_id_file" ]; then
            rm -f "$codespace_id_file"
            echo -e "  🗑️  Файл ID Codespace удален"
        fi
    else
        echo -e "  ❌ Не удалось удалить Codespace ${codespace_id}"
        failed_stops=$((failed_stops + 1))
    fi
}

# Функция для полной очистки
full_cleanup() {
    echo -e "${YELLOW}🧹 Полная очистка...${NC}"
    
    # Удаляем все Codespaces
    for node_id in "${!NODES[@]}"; do
        local codespace_id_file="logs/${node_id}_codespace_id"
        if [ -f "$codespace_id_file" ]; then
            local codespace_id=$(cat "$codespace_id_file")
            if [ -n "$codespace_id" ] && [ "$codespace_id" != "null" ]; then
                delete_codespace "$node_id" "$codespace_id"
            fi
        fi
    done
    
    # Очищаем все временные файлы
    echo -e "${BLUE}🧹 Очистка временных файлов...${NC}"
    
    # Удаляем файлы активности
    rm -f logs/*_active
    echo -e "  ✅ Файлы активности удалены"
    
    # Удаляем PID файлы
    rm -f logs/*.pid
    echo -e "  ✅ PID файлы удалены"
    
    # Удаляем файлы ID Codespaces
    rm -f logs/*_codespace_id
    echo -e "  ✅ Файлы ID Codespaces удалены"
    
    # Очищаем логи узлов (оставляем только основные логи)
    if [ -d "logs" ]; then
        find logs -name "node_*.log" -type f -delete
        echo -e "  ✅ Логи узлов удалены"
    fi
    
    echo -e "${GREEN}✅ Полная очистка завершена${NC}"
}

# Функция для проверки статуса после остановки
check_cleanup_status() {
    echo -e "${BLUE}🔍 Проверка статуса после остановки...${NC}"
    
    local remaining_files=0
    local remaining_codespaces=0
    
    # Проверяем оставшиеся файлы
    for node_id in "${!NODES[@]}"; do
        local active_file="logs/${node_id}_active"
        local codespace_id_file="logs/${node_id}_codespace_id"
        
        if [ -f "$active_file" ]; then
            remaining_files=$((remaining_files + 1))
            echo -e "  ⚠️  Остался файл активности: ${active_file}"
        fi
        
        if [ -f "$codespace_id_file" ]; then
            remaining_files=$((remaining_files + 1))
            local codespace_id=$(cat "$codespace_id_file")
            echo -e "  ⚠️  Остался файл ID Codespace: ${codespace_id_file} (${codespace_id})"
            
            # Проверяем, существует ли Codespace
            if [ -n "$codespace_id" ] && [ "$codespace_id" != "null" ]; then
                local status=$(get_codespace_status "$codespace_id")
                if [ "$status" != "Deleted" ] && [ "$status" != "Failed" ]; then
                    remaining_codespaces=$((remaining_codespaces + 1))
                    echo -e "    📊 Статус Codespace: ${status}"
                fi
            fi
        fi
    done
    
    echo ""
    echo -e "${BLUE}📊 Статистика очистки:${NC}"
    echo -e "  Осталось файлов: ${remaining_files}"
    echo -e "  Осталось Codespaces: ${remaining_codespaces}"
    
    if [ $remaining_files -eq 0 ] && [ $remaining_codespaces -eq 0 ]; then
        echo -e "${GREEN}✅ Очистка полностью завершена${NC}"
    else
        echo -e "${YELLOW}⚠️  Очистка не полностью завершена${NC}"
    fi
}

# Функция для остановки всех узлов
stop_all_nodes() {
    echo -e "${BLUE}🛑 Останавливаем все узлы...${NC}"
    echo ""
    
    for node_id in "${!NODES[@]}"; do
        IFS=':' read -r port shard_id location <<< "${NODES[$node_id]}"
        stop_node "$node_id" "$port" "$shard_id"
        total_nodes=$((total_nodes + 1))
    done
    
    echo ""
    echo -e "${BLUE}📊 Статистика остановки:${NC}"
    echo -e "  Всего узлов: ${total_nodes}"
    echo -e "  Успешно остановлено: ${GREEN}${stopped_nodes}${NC}"
    echo -e "  Ошибок остановки: ${RED}${failed_stops}${NC}"
}

# Обработка сигналов для корректного завершения
trap 'echo -e "\n${YELLOW}⚠️  Получен сигнал прерывания. Завершаем...${NC}"; exit 1' SIGINT SIGTERM

# Основная логика
main() {
    echo -e "${BLUE}🔧 Инициализация GitHub API...${NC}"
    init_github_api
    
    echo ""
    echo -e "${BLUE}📋 Конфигурация сети:${NC}"
    for node_id in "${!NODES[@]}"; do
        IFS=':' read -r port shard_id location <<< "${NODES[$node_id]}"
        echo -e "  ${node_id}: порт ${port}, shard ${shard_id}, локация ${location}"
    done
    echo ""
    
    # Останавливаем все узлы
    stop_all_nodes
    
    echo ""
    echo -e "${BLUE}🧹 Очистка ресурсов...${NC}"
    
    # Спрашиваем пользователя о полной очистке
    echo -e "${YELLOW}💡 Хотите ли вы полностью удалить все Codespaces? (y/N)${NC}"
    read -r response
    
    if [[ "$response" =~ ^[Yy]$ ]]; then
        echo -e "${BLUE}🗑️  Выполняем полную очистку...${NC}"
        full_cleanup
    else
        echo -e "${BLUE}🧹 Выполняем базовую очистку...${NC}"
        # Удаляем только файлы активности
        rm -f logs/*_active
        rm -f logs/*.pid
        echo -e "  ✅ Базовая очистка завершена"
    fi
    
    echo ""
    
    # Проверяем статус очистки
    check_cleanup_status
    
    echo ""
    if [ $failed_stops -eq 0 ]; then
        echo -e "${GREEN}🎉 Все узлы успешно остановлены!${NC}"
    else
        echo -e "${YELLOW}⚠️  ${failed_stops} узлов не удалось корректно остановить.${NC}"
    fi
    
    echo "========================================="
}

# Запуск основной логики
main "$@"
